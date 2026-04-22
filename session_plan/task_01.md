Title: Graceful handling of missing environment variables
Files: src/main.rs
Issue: none

Add validation at application startup to ensure the required environment variables (`BASE_URL`, `MODEL`, `API_KEY`) are present. If any are missing, log a clear error message to stderr and exit the program with a non‑zero status instead of panicking later when the LLM provider is constructed. Update the startup flow in `src/main.rs` to perform this check before creating the `LlmConfig` or the `Agent`. Ensure the user gets a helpful message and `--help` still works.
