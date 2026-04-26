Title: Add default config option for assessment output path
Files: src/config.rs, tests/config.rs
Issue: none

Description:
- In `src/config.rs`, extend the `Config` struct with a new field `assessment_output_path: PathBuf`.
- Set its default value to `PathBuf::from("target/assessment.json")` using the existing `Default` implementation.
- Ensure the field is serialized/deserialized via `serde` like other config options.
- Add a unit test in `tests/config.rs` that loads the default configuration (e.g., via `Config::default()`) and asserts that `assessment_output_path` equals the expected default path.
- Keep modifications limited to the two listed files.
