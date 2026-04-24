Title: Add git staging and commit CLI flags
Files: src/main.rs
Issue: none

Add two optional command‑line flags to the agent:
- `--stage-all` – calls `git::stage_all()` before the app runs, printing any error and exiting with code 1.
- `--git-commit <msg>` – after optional staging, runs `git::commit(msg)`. If it fails, print the error and exit with code 1.

Implementation steps:
1. Extend the `Args` struct (lines around 48‑64) with:
   ```rust
   #[arg(long, action = ArgAction::SetTrue)]
   stage_all: bool,

   #[arg(long, value_name = "msg")]
   git_commit: Option<String>,
   ```
2. In `main()` after environment validation, check `args.stage_all` and call `git::stage_all()`; on error, `eprintln!("{msg}"); std::process::exit(1);`.
3. If `args.git_commit` is `Some(msg)`, call `git::commit(&msg)`, handling errors similarly.
4. Add a small test module at the bottom of `src/main.rs` (under `#[cfg(test)]`) to verify argument parsing produces the expected values.
5. Ensure the new imports (`use crate::git;`) are added.

This provides basic git integration, a capability Claude Code offers, and improves self‑evolution by exposing version‑control operations directly.
