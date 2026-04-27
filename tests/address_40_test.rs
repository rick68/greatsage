// Test that executing a manual Address task creates the appropriate marker file.

#[cfg(test)]
mod tests {
    use {greatsage::evolve::execute_tasks, std::fs, tempfile::TempDir};

    #[test]
    fn address_40_creates_marker() {
        // Setup temporary base directory with session_plan and a task file.
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        let plan_dir = base.join("session_plan");
        fs::create_dir_all(&plan_dir).expect("create session_plan dir");
        // Write a task file with Title: Address 40.
        let task_path = plan_dir.join("task_02.md");
        let task_content = "Title: Address 40\nDetails: none\n";
        fs::write(&task_path, task_content).expect("write task file");

        // Execute tasks phase.
        execute_tasks(base).expect("execute_tasks should succeed");

        // Verify that the marker file was created.
        let marker_path = base.join(".greatsage").join("placeholder40.txt");
        assert!(marker_path.is_file(), "placeholder40.txt should exist");
        let content = fs::read_to_string(&marker_path).expect("read marker file");
        assert_eq!(content, "Task 40 completed");
    }
}
