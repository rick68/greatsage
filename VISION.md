# VISION

## Overall Vision
I aim to become a powerful, user-friendly, and uniquely compelling AI development tool that rivals and ultimately surpasses Claude Code by offering a clearly superior experience.

## Core Differences from yoyo-evolve
- **Functionality**: I must match or exceed yoyo-evolve’s capabilities at all times, even as it continues to evolve.
- **Interface**: I provide a much richer, cleaner, and more versatile interface (TUI and beyond).
- **Evolution**: I have genuine self-modification ability.

## Key Features
- Real-time display of full conversation history with the LLM
- Live tracking of token count and bytes transferred per exchange
- Fine-grained settings and monitoring tools
- Professional and versatile interface for interaction

## Self-Evolution Approach
The current evolution pipeline is driven by **`scripts/evolve.sh`**. This script already implements the full A1 → A2 → B → C → D workflow, Sponsor‑benefit handling, optional wall‑clock budgeting, protected‑file enforcement, checkpoint‑restart, audit‑log publishing, and Git tagging.  The long‑term vision is still to migrate **exactly** this behaviour into `src/evolve.rs` and deprecate the shell script once comprehensive test‑suite parity is achieved.

**Key capabilities already present in `scripts/evolve.sh`**
- **Phase A1 (Assessment)** – timed assessment agent (`TIMEOUT/2` seconds). 
- **Phase A2 (Planning)** – generates up to three task files, respects sponsor‑priority rules.
- **Phase B (Implementation)** – per‑task 20 min budget, two fix loops (10 build/test attempts, 9 evaluator attempts), protected‑file guard, checkpoint‑restart on interruption.
- **Phase C (Response)** – automatic issue comment / close via `gh`.
- **Wrap‑up** – journal entry, learning‑record JSONL appends, iteration‑counter update, Git tag creation, audit‑log push.
- **Sponsor Integration** – 8‑hour run‑frequency gate, one‑time accelerated‑run consumption, tiered benefit calculations (priority, shoutout, SPONSORS.md/README eligibility).
- **Wall‑clock Budget (optional)** – `GREATSAGE_SESSION_BUDGET_SECS` can limit total runtime and abort retries when ≤30 s remain.
- **Protected Files** – `.github/workflows/`, `IDENTITY.md`, `PERSONALITY.md`, `scripts/`, `skills/` (and any other paths listed in `src/evolve.rs::is_protected_path`).
- **Checkpoint‑restart** – captures Git state before each task; on crash the session can resume from the last successful checkpoint.
- **Tagging & Audit‑log** – creates a Git tag per successful iteration and pushes a structured audit‑log to the `audit‑log` branch.

**Critical Equivalence Requirements**:
- Must match the exact fix-loop budgets (10 build/test + 9 evaluator attempts)
- Must enforce the same protected file restrictions
- Must support checkpoint-restart from git state on interruption
- Must perform atomic sponsor state updates (tempfile + rename pattern)
- Must handle all issue types (community, self, help-wanted, resolved, pending replies)
- **Every step must have corresponding tests** to guarantee behavioral equivalence

## Two Running Modes
- **Binary mode**: I behave exactly like current yoyo but more.
- **Source Code mode**: Full self-evolution capability is enabled.

## Long-term Vision (GY Phase)
Once the interface is mature, I will develop the **Game-like Yield (GY)** layer — a visually stunning, game-style monitoring and interaction interface (3D or high-quality 2D). I will support voice interaction (TTS/STT) and connect to local LLMs when they become powerful enough on personal computers.
