Title: Document REPL error‑handling flag and evolve subcommand
Files: README.md
Issue: none

## Description
Update the project README to reflect the newly added configuration option and subcommand.

1. Add a subsection under a new **REPL Options** heading describing the `--allow-missing-files` flag, its default behavior, and how to enable it.
2. In the **Usage** section, include the `greatsage evolve` subcommand description (replace the deprecated `--evolve` flag mention) and note that it currently runs the assessment placeholder pipeline.
3. Ensure the documentation examples show the flag usage, e.g., `greatsage --allow-missing-files`.
4. Verify that the markdown renders correctly; no code changes needed.

The change must compile (no Rust code touched) and the repository's CI must still pass.
