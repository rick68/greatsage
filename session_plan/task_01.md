Title: Add --config flag and load configuration file
Files: src/cli.rs, src/config.rs, src/main.rs
Issue: none

Implement a new optional command‑line flag `--config <PATH>` that allows the user to specify a configuration file. If omitted, default to `$HOME/.greatsage.toml`. The CLI should parse this flag, pass the path to `Config::load`, and make the resulting `Config` available to the application (e.g., as a resource or global). Update `Config` to include default values for any missing fields, and ensure errors when the file cannot be read are reported cleanly.

Steps:
1. In `src/cli.rs` add a `--config` option (String, optional) with Clap documentation.
2. In `src/main.rs` after argument parsing, call `Config::load` with the provided path or default, handling Result and exiting with a clear error message on failure.
3. Store the loaded config (e.g., via a global static, or inject as a Bevy resource if applicable).
4. Update unit tests for CLI parsing to cover the new flag.
5. Ensure documentation (`GREATSAGE.md` or README) mentions the new flag.
