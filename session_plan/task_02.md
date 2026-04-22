Title: Add positional prompt argument support
Files: src/main.rs
Issue: none

Modify the CLI handling so that a bare positional argument is interpreted as a prompt, matching the existing `--prompt` flag. Update the `Args` struct to include an optional `positional_prompt: Option<String>` using `#[arg(value_name = "prompt", required = false)]`. In `main`, after parsing, if `args.prompt` is None but `args.positional_prompt` is Some, use that value as the prompt. Ensure existing behavior for `--prompt` and REPL mode remains unchanged. Update help output accordingly. Add a test in `src/main.rs` (under `#[cfg(test)]`) that verifies a positional argument is correctly parsed into the prompt field.
