Title: Add evolve subcommand placeholder
Files: src/main.rs, src/evolve.rs
Issue: none

Add a new CLI flag `--evolve` (or subcommand `evolve`) to `Args` in `src/main.rs`. When this flag is present, the program should call a new module `src/evolve.rs` with a function `run_evolve()` that currently just prints a placeholder message and exits with code 0. Create `src/evolve.rs` containing a public `pub fn run_evolve() -> Result<(), Box<dyn std::error::Error>>` that writes to stdout: "[greatsage] evolve mode placeholder" and returns Ok(()). Ensure the flag is parsed and integrated without breaking existing functionality. Keep changes limited to these two files and compile successfully.
