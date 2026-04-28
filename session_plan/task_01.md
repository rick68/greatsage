Title: Add --error-handling flag to REPL and align assessment detection
Files: src/main.rs, src/cli.rs, src/evolve.rs
Issue: none

## Description
The self‑assessment still reports a missing "error‑handling" flag in the REPL, even though a `--check` flag exists. Implement a dedicated `--error-handling` flag that:
- Enables runtime validation of REPL user input (ensures commands are known and arguments are present).
- Returns a clear error message and exits with a non‑zero status when validation fails.
- Is detected by the assessment step as present, removing the gap.

### Steps
1. **src/cli.rs** – Add a new boolean argument `--error-handling` (short `-e`).
2. **src/main.rs** – When the flag is present, wrap the REPL command‑processing loop with validation logic. For each entered command, match against a whitelist of known commands; on unknown command, print an error and exit with code 1.
3. **src/evolve.rs** – Update the assessment routine to look for the new flag (`--error-handling`) instead of the old `--check` flag.
4. Add unit tests verifying:
   - Flag parses correctly via CLI.
   - Validation rejects an unknown command when the flag is on.
   - Validation passes for known commands.
   - Assessment no longer reports the missing flag.
5. Update **README.md** to document the new flag and its purpose.

The change touches exactly three source files and can be verified with `cargo test` and a manual run of the binary with the flag.
