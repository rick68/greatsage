// Test for Address 40 task execution

#[cfg(test)]
mod tests {
    use {crate::evolve, std::fs, tempfile::TempDir};

    #[test]
    fn test_execute_task_address_40() {
        // Setup temporary base directory with a session_plan and a task file titled "Address 40".
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        let plan_dir = base.join("session_plan");
        () = fs::create_dir_all(&plan_dir).expect("create session_plan dir");
        let task_path = plan_dir.join("task_01.md");
        let task_content = "Title: Address 40\nDetails: none\n";
        () = fs::write(&task_path, task_content).expect("write task file");
        // Execute tasks phase.
        () = evolve::execute_tasks(base).expect("execute_tasks should succeed");
        // Verify evolve.log contains the task title.
        let log_path = base.join(".greatsage").join("evolve.log");
        assert!(log_path.is_file(), "evolve.log should be created");
        let log_content = fs::read_to_string(&log_path).expect("read evolve.log");
        assert!(log_content.contains("Address 40"), "log should contain task title");
        // Verify placeholder file created.
        let placeholder_path = base.join(".greatsage").join("placeholder40.txt");
        assert!(placeholder_path.is_file(), "placeholder40.txt should be created");
        let marker = fs::read_to_string(&placeholder_path).expect("read placeholder file");
        assert_eq!(marker, "Task 40 completed");
    }
}
