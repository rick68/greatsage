// Test execution of Placeholder Task 1 creates marker file

#[cfg(test)]
mod tests {
    use {crate::evolve, std::fs, tempfile::TempDir};

    #[test]
    fn test_placeholder_task_1_marker_created() {
        // Setup temporary base directory with a session_plan and task 1 file.
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        let plan_dir = base.join("session_plan");
        () = fs::create_dir_all(&plan_dir).expect("create session_plan");
        // Create task_01.md with title Placeholder Task 1
        let task_path = plan_dir.join("task_01.md");
        let task_content = "Title: Placeholder Task 1\nDetails: none\n";
        () = fs::write(&task_path, task_content).expect("write task file");
        // Run execute_tasks
        () = evolve::execute_tasks(base).expect("execute_tasks should succeed");
        // Verify placeholder1.txt marker file exists and contains expected content
        let marker_path = base.join(".greatsage").join("placeholder1.txt");
        assert!(marker_path.is_file(), "placeholder1.txt should be created");
        let marker_content = fs::read_to_string(&marker_path).expect("read marker");
        assert_eq!(marker_content, "Task 1 completed");
    }
}
