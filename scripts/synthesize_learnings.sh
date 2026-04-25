#!/usr/bin/env bash
# scripts/synthesize_learnings.sh — Synthesize raw JSONL archives into human-readable active context files.
# Ported from yoyo-evolve synthesize.yml and optimized for local use only.
# Usage: ./scripts/synthesize_learnings.sh

set -euo pipefail

# Source shared helpers (provides REPO, BOT_LOGIN, BOT_SLUG)
source "$(dirname "$0")/common.sh"

# Configuration
MODEL="${MODEL:-claude-opus-4-7}"
GREATSAGE_BIN="${GREATSAGE_BIN:-./target/debug/greatsage}"

# Build if binary is missing
if [ ! -x "$GREATSAGE_BIN" ]; then
    echo "→ Building greatsage binary..."
    cargo build --quiet
    GREATSAGE_BIN="./target/debug/greatsage"
fi

mkdir -p memory

# Only run if there are new entries (safe check for missing files)
LEARNINGS_COUNT=0
if [ -f memory/learnings.jsonl ]; then
    LEARNINGS_COUNT=$(grep -c '.' memory/learnings.jsonl 2>/dev/null || echo 0)
fi

SOCIAL_COUNT=0
if [ -f memory/social_learnings.jsonl ]; then
    SOCIAL_COUNT=$(grep -c '.' memory/social_learnings.jsonl 2>/dev/null || echo 0)
fi

if [ "$LEARNINGS_COUNT" -eq 0 ] && [ "$SOCIAL_COUNT" -eq 0 ]; then
    echo "→ No entries in learnings.jsonl or social_learnings.jsonl. Skipping synthesis."
    exit 0
fi

echo "→ Synthesizing $LEARNINGS_COUNT learnings + $SOCIAL_COUNT social learnings in repo $REPO..."

# Create temporary backup directory (never touches repo files)
BACKUP_DIR=$(mktemp -d -t greatsage-synth-backup-XXXXXX)
trap "rm -rf $BACKUP_DIR" 0 1 2 3 15
echo "→ Temporary backup directory created: $BACKUP_DIR"

# Backup active files
cp memory/active_learnings.md "$BACKUP_DIR/active_learnings.md.bak" 2>/dev/null || true
cp memory/active_social_learnings.md "$BACKUP_DIR/active_social_learnings.md.bak" 2>/dev/null || true

# ── Synthesize active_learnings.md ──
echo "  → Synthesizing active_learnings.md..."
PROMPT=$(mktemp)
trap "rm -f $PROMPT" 0 1 2 3 15
cat > "$PROMPT" <<'SYNTHEOF'
You are synthesizing greatsage's learning archive into an active context file.

Read memory/learnings.jsonl (the full archive) and regenerate memory/active_learnings.md.

Apply iteration-weighted compression tiers:
- **Recent (last 2 weeks):** Render each entry as full markdown (## Lesson: title, **Iteration:** N | **Date:** YYYY-MM-DDThh:mmZ | **Source:** source, **Context:** context, takeaway)
- **Medium (2-8 weeks old):** Condense each entry to 1-2 sentences under its title
- **Old (8+ weeks):** Group entries by theme into ## Wisdom: [theme] summaries (2-3 sentences per group)

Keep total under ~200 lines. Preserve the most actionable and unique insights.

Use the write_file tool to write the result to memory/active_learnings.md. Start the file with:
# Active Learnings

Self-reflection — what I've learned about how I work, what I value, and how I'm growing.

Do not output the file content as text in your response. Only confirm the file was written.
SYNTHEOF

timeout 180 "$GREATSAGE_BIN" --model "$MODEL" --skills ./skills < "$PROMPT" || {
    echo "WARNING: Learnings synthesis failed — restoring backup."
    [ -f "$BACKUP_DIR/active_social_learnings.md.bak" ] && cp "$BACKUP_DIR/active_social_learnings.md.bak" memory/active_social_learnings.md
}

# ── Synthesize active_social_learnings.md ──
echo "  → Synthesizing active_social_learnings.md..."
PROMPT=$(mktemp)
trap "rm -f $PROMPT" 0 1 2 3 15
cat > "$PROMPT" <<'SYNTHEOF'
You are synthesizing greatsage's social learning archive into an active context file.

Read memory/social_learnings.jsonl (the full archive) and regenerate memory/active_social_learnings.md.

Apply time-weighted compression tiers:
- **Recent (last 2 weeks):** Render each entry as a full bullet with metadata
- **Medium (2-8 weeks old):** Keep insight only, drop metadata
- **Old (8+ weeks):** Group by theme into ## Wisdom: [theme] summaries (2-3 sentences per group)

Keep total under ~100 lines.

Use the write_file tool to write the result to memory/active_social_learnings.md. Start the file with:
# Active Social Learnings

What I've learned about people from talking with them.

Do not output the file content as text in your response. Only confirm the file was written.
SYNTHEOF

timeout 180 "$GREATSAGE_BIN" --model "$MODEL" --skills ./skills < "$PROMPT" || {
    echo "WARNING: Learnings synthesis failed — restoring backup."
    [ -f "$BACKUP_DIR/active_social_learnings.md.bak" ] && cp "$BACKUP_DIR/active_social_learnings.md.bak" memory/active_social_learnings.md
}

echo "✅ Synthesis complete — active_learnings.md and active_social_learnings.md are now up to date."
