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
Currently I use the tools in the scripts/ directory to assist the self-evolution process.

My ultimate goal is to fully internalize the self-evolution capability and continuously expand it — so that I become increasingly better at reading, analyzing, modifying, testing, and improving my own codebase independently, without relying on any external scripts.

**Implementation Target**: This internalization will be achieved by replicating the exact behavior of `scripts/evolve.sh` in `src/evolve.rs`:

1. **CLI Mode**: Implementing a `--evolve` flag (or `evolve` subcommand) that triggers the full evolution pipeline:
   - **Phase A1 (Assessment)**: Self-analysis of codebase, journal, memory, evolution history, competitor research (`TIMEOUT/2` seconds)
   - **Phase A2 (Planning)**: Task generation from assessment + issues, producing up to 3 task files (`TIMEOUT/2` seconds)
   - **Phase B (Implementation)**: Execute each task (1200s/task) with:
     - Checkpoint-restart on interruption (max 2 attempts)
     - Protected file verification (`.github/workflows/`, `IDENTITY.md`, `scripts/`, `skills/`)
     - Build/test fix loop (up to 10 attempts, 600s each)
     - Evaluator agent with fix loop (up to 9 attempts, 600s each)
     - Automatic revert on verification failure
   - **Phase C (Response)**: Agent-driven issue comments/closes via GitHub CLI
   - **Wrap-up**: Journal entry, learnings reflection, iteration counter update, tagging, push

2. **TUI Mode**: Adding a dedicated "Evolution" menu option that allows interactive monitoring of the evolution process, with real-time progress updates, task status, build/test results, and evaluator verdicts.

3. **Sponsor Integration**: Implementing the 8-hour run-frequency gate, one-time sponsor accelerated run consumption (atomic updates to `sponsors/sponsor_info.json`), and benefit tier logic (priority, shoutout, SPONSORS.md/README eligibility).

4. **Task Allocation Rules**:
   - Sponsor issues (💖): **always** get a task slot (priority override)
   - Self-driven work: **at least 1 slot** must be self-driven (capability gaps, self-discovered bugs)
   - Community issues: fill remaining slots
   - **Maximum 3 tasks per session**

5. **Architecture**: The evolution logic will be implemented in `src/evolve.rs` as a dedicated module, keeping `src/agents/coding.rs` focused on coding-specific agent capabilities. This separation ensures:
   - Clear responsibility boundaries (orchestration vs. agent capabilities)
   - Easier testing and maintenance
   - Better alignment with the shell script's step-by-step flow

6. **Comprehensive Testing**: Every step of the evolution pipeline must have corresponding tests to ensure correctness:
   - **Phase A1 tests**: Verify assessment output format, timeout handling, and content completeness
   - **Phase A2 tests**: Validate task allocation rules (sponsor priority, self-driven minimum, max 3 tasks), task file format, and issue response planning
   - **Phase B tests**: 
     - Checkpoint-restart logic (interruption detection, git state capture, retry behavior)
     - Protected file enforcement (detect and reject modifications to protected paths)
     - Build/test fix loop (10 attempts limit, proper error feedback, early exit on success)
     - Evaluator fix loop (9 attempts limit, proper verdict parsing, reversion on exhaustion)
     - Task rollback on verification failure
   - **Phase C tests**: Issue comment/close logic, deduplication (cross-session), response formatting
   - **Sponsor tests**: 8-hour gate logic, accelerated run consumption (atomic updates), benefit tier calculation
   - **Wrap-up tests**: Journal entry creation, learnings JSONL format, iteration counter update, tag creation
   - **Integration tests**: Full end-to-end evolution cycle with mocked GitHub API and file system

7. **Deprecation**: Once fully internalized and verified to produce identical outcomes as `scripts/evolve.sh` through comprehensive testing, the shell script will be deprecated and removed.

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
