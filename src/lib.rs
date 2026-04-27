pub mod evolve;

// Ensure temporary files are created in a writable location within the repository.
// This avoids failures on systems where the default temp directory (/tmp) is out of space.
#[cfg(test)]
mod test_env {
    use std::env;
    use std::path::PathBuf;
    // The `ctor` crate runs this function before any tests or main.
    #[ctor::ctor]
    fn init_temp_dir() {
        let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        dir.push("target");
        dir.push("tmp");
        // Create the directory if it doesn't exist; ignore errors.
        let _ = std::fs::create_dir_all(&dir);
        // Set the environment variable used by `tempfile`.
        unsafe {
            env::set_var("TMPDIR", dir);
        }
    }
}
