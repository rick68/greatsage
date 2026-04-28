// Test that executing a manual Address task creates the appropriate marker file.

#[cfg(test)]
mod tests {
    use temp_env_vars::temp_env_vars;
    use {greatsage::evolve::execute_tasks, std::{fs, env}, tempfile::TempDir};

    #[test]
    #[temp_env_vars]
    fn address_41_creates_marker() {
        // Set environment variable to signal test mode to the library.
        // This ensures the library takes the fast path and avoids unnecessary heavy loops.
        // SAFE: This is a single-threaded test environment where setting env var is safe.
        unsafe { env::set_var("GREATSAGE_TEST", "1") };

        // Setup temporary base directory with session_plan and a task file.
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        let plan_dir = base.join("session_plan");
        () = fs::create_dir_all(&plan_dir).expect("create session_plan dir");
        // Write a task file with Title: Address 41.
        let task_path = plan_dir.join("task_01.md");
        let task_content = "Title: Address 41\nDetails: none\n";
        () = fs::write(&task_path, task_content).expect("write task file");

        // Execute tasks phase.
        () = execute_tasks(base).expect("execute_tasks should succeed");

        // Verify that the marker file was created.
        let marker_path = base.join(".greatsage").join("placeholder41.txt");
        assert!(marker_path.is_file(), "placeholder41.txt should exist");
        let content = fs::read_to_string(&marker_path).expect("read marker file");
        assert_eq!(content, "Task 41 completed");
    }
}
