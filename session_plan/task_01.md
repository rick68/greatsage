Title: Add Git commit helper function
Files: src/git.rs
Issue: none

Implement a utility function `fn commit_changes(message: &str) -> Result<(), Box<dyn std::error::Error>>` that runs `git add -A && git commit -m "..."` using `std::process::Command`. The function should capture stdout/stderr, return `Ok(())` on success, or return an error containing the command's stderr if it fails. This provides basic Git integration for the evolve pipeline.
