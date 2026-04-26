// Test execution creates placeholder file for Placeholder Task 3

#[cfg(test)]
mod tests {
    use {crate::evolve::execute_tasks, std::fs, tempfile::TempDir};

    #[test]
    fn test_placeholder_task_3_execution_creates_marker() {
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        let plan_dir = base.join("session_plan");
        () = fs::create_dir_all(&plan_dir).expect("create session_plan dir");
        let task_path = plan_dir.join("task_03.md");
        let task_content = "Title: Placeholder Task 3\nDetails: none\n";
        () = fs::write(&task_path, task_content).expect("write task file");
        // Execute tasks.
        () = execute_tasks(base).expect("execute_tasks should succeed");
        // Verify placeholder marker file.
        let placeholder_path = base.join(".greatsage").join("placeholder3.txt");
        assert!(
            placeholder_path.is_file(),
            "placeholder3.txt should be created"
        );
        let marker = fs::read_to_string(&placeholder_path).expect("read placeholder file");
        assert_eq!(marker, "Task 3 completed");
    }
}
