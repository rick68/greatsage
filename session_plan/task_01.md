Title: Refactor environment variable validation for clarity
Files: src/main.rs
Issue: none

Refactor the `validate_env_vars` function to use clear, idiomatic Rust code. Replace the unconventional `() = missing.push(*var)` line and the generic `missing.join::<&str>(", ")` call with standard `missing.push(var);` and `missing.join(", ")`. Ensure the function still returns the same error messages and passes existing tests. Update any related comments if needed.
