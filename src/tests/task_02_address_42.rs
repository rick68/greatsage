// Test execution of generic Address 42 task creates correct marker file

#[cfg(test)]
mod tests {
    use {crate::evolve, std::fs, tempfile::TempDir};

    #[test]
    fn test_address_42_marker_created() {
        // Setup temporary base directory with a session_plan and task file for Address 42.
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        let plan_dir = base.join("session_plan");
        fs::create_dir_all(&plan_dir).expect("create session_plan");
        let task_path = plan_dir.join("task_02.md");
        let task_content = "Title: Address 42\nDetails: none\n";
        fs::write(&task_path, task_content).expect("write task file");
        // Run execute_tasks (test mode, so fast path).
        evolve::execute_tasks(base).expect("execute_tasks should succeed");
        // Verify placeholder42.txt marker file exists with expected content.
        let marker_path = base.join(".greatsage").join("placeholder42.txt");
        assert!(marker_path.is_file(), "placeholder42.txt should be created");
        let marker_content = fs::read_to_string(&marker_path).expect("read marker");
        assert_eq!(marker_content, "Task 42 completed");
        // Verify log contains the task title.
        let log_path = base.join(".greatsage").join("evolve.log");
        let log_content = fs::read_to_string(log_path).expect("read evolve.log");
        assert!(log_content.contains("Address 42"), "log should contain task title");
    }
}
