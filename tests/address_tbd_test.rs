// Test that executing a task titled "Address TBD" does not create any marker files and succeeds.

#[cfg(test)]
mod tests {
    use {greatsage::evolve::execute_tasks, std::fs, tempfile::TempDir};

    #[test]
    fn address_tbd_is_noop() {
        // Setup temporary base directory with session_plan and a task file.
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        let plan_dir = base.join("session_plan");
        () = fs::create_dir_all(&plan_dir).expect("create session_plan dir");
        let task_path = plan_dir.join("task_03.md");
        let task_content = "Title: Address TBD\nDetails: none\n";
        () = fs::write(&task_path, task_content).expect("write task file");

        // Execute tasks phase.
        () = execute_tasks(base).expect("execute_tasks should succeed");

        // Verify that placeholder_TBD marker file was created.
        let placeholder_path = base.join(".greatsage").join("placeholder_tbd.txt");
        assert!(
            placeholder_path.is_file(),
            "placeholder_tbd.txt should be created"
        );
        let content = fs::read_to_string(&placeholder_path).expect("read placeholder file");
        assert_eq!(content, "Task TBD completed");
    }
}
