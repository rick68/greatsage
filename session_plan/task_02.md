Title: Add --provider flag for selecting LLM provider
Files: src/cli.rs, src/main.rs
Issue: none

Implement a new optional command‑line flag `--provider <PROVIDER>` that allows the user to choose which LLM provider to use (e.g., `anthropic`, `openai`, `groq`). The flag should map to the existing `Provider` enum in `src/providers.rs`.

Steps:
1. In `src/cli.rs` add a `--provider` option (String, optional) with Clap documentation and possible values derived from the `Provider` enum.
2. In `src/main.rs` after parsing CLI arguments, convert the provider string into the `Provider` enum (defaulting to `Provider::Anthropic` if not supplied). Handle invalid values with a clear error message.
3. Pass the selected provider into the application setup (e.g., store it in a Bevy resource or a global config struct) so that downstream code can access it.
4. Update existing unit tests for CLI parsing to include scenarios with the new flag, and add a test verifying the default provider when the flag is omitted.
5. Update documentation (README or GREATSAGE.md) to list the new flag and its usage.

This task touches at most two source files and keeps the change scoped to command‑line handling and configuration setup.
