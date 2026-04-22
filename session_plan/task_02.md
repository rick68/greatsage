Title: Add LLM request retry logic
Files: src/agents/coding.rs, src/agents/mod.rs
Issue: none

Implement a configurable retry mechanism for LLM API calls. Add a constant `MAX_RETRY_ATTEMPTS: usize` (default 3) in `src/agents/mod.rs` or a config struct. In `coding.rs` where LLM requests are made (e.g., `client.send_message`), wrap the call in a loop that retries on transient errors (network failures, HTTP 5xx). After exhausting retries, return a clear error to the user. Log each retry attempt. Add a unit test mocking a failing LLM call that succeeds on the second attempt to verify the retry behavior.
