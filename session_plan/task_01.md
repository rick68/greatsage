Title: Fix REPL check flag persistence test failure
Files: src/main.rs, src/config.rs, src/tests.rs
Issue: none

Implement fixes so that the test `persist_repl_error_handling::tests::test_check_flag_persists_to_config` passes in CI. Likely adjust the code handling working‑directory detection or modify the test setup to create a temporary directory and ensure the config file is written there. Ensure the code correctly determines the repository root or fallback to current directory when not in a git repo, and that the test creates the appropriate environment.
