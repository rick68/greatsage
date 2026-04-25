Title: Document --evolve flag and sponsor gating in README
Files: README.md
Issue: none

Update the README to reflect the new `--evolve` flag functionality and sponsor gating:
- Add a description of the `--evolve` subcommand, its purpose (self‑evolution pipeline), and how it is invoked.
- Explain the 8‑hour runtime gate, the optional `--sponsor` flag to bypass it, and the behavior when the gate blocks execution.
- Provide a short example command line invocation.
- Mention that the evolve pipeline will generate task files in `session_plan/` and that further details are in the `src/evolve.rs` implementation.
- Keep the style consistent with existing README sections.

No code changes beyond the documentation file.
