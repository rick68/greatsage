#!/usr/bin/env bash
# common.sh — shared auto-detection for fork-friendly operation.
# Source this from evolve.sh, social.sh, daily_diary.sh, etc.
# Exports: REPO, BOT_LOGIN, BOT_SLUG
# All variables have sensible defaults for yoyo-evolve; forks override via env.

_COMMON_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_REPO_ROOT="$(cd "$_COMMON_DIR/.." && pwd)"

# Auto-detect repo from git remote if not set via env
if [ -z "${REPO:-}" ]; then
    REPO=$(git remote get-url origin 2>/dev/null | sed -E 's|.*github\.com[:/]||; s|\.git$||')
fi
if [ -z "${REPO:-}" ]; then
    echo "FATAL: Could not detect REPO from git remote. Set REPO env var." >&2
    exit 1
fi

# Bot identity — detected from GitHub App in CI, defaults for local runs.
# In CI, both BOT_LOGIN and BOT_SLUG are set by the workflow's "Detect bot identity" step.
# These defaults only apply for local runs.
BOT_SLUG="${BOT_SLUG:-greatsage}"
BOT_LOGIN="${BOT_LOGIN:-${BOT_SLUG}[bot]}"

cd $_REPO_ROOT
