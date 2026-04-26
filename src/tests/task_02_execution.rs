// Test execution of Placeholder Task 2 creates marker file

#[cfg(test)]
mod tests {
    use {crate::evolve, std::fs, tempfile::TempDir};

    #[test]
    fn test_placeholder_task_2_marker_created() {
        // Setup temporary base directory with a session_plan and task 2 file.
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        let plan_dir = base.join("session_plan");
        fs::create_dir_all(&plan_dir).expect("create session_plan");
        // Create task_02.md with title Placeholder Task 2
        let task_path = plan_dir.join("task_02.md");
        let task_content = "Title: Placeholder Task 2\nDetails: none\n";
        fs::write(&task_path, task_content).expect("write task file");
        // Run execute_tasks
        evolve::execute_tasks(base).expect("execute_tasks should succeed");
        // Verify placeholder2.txt marker file exists and contains expected content
        let marker_path = base.join(".greatsage").join("placeholder2.txt");
        assert!(marker_path.is_file(), "placeholder2.txt should be created");
        let marker_content = fs::read_to_string(&marker_path).expect("read marker");
        assert_eq!(marker_content, "Task 2 completed");
    }
}
