// Test that planning_phase generates Placeholder Task 3 correctly

#[cfg(test)]
mod tests {
    use {greatsage::evolve::planning_phase, std::fs, tempfile::TempDir};

    #[test]
    fn placeholder_task_3_generated() {
        // Setup temporary base directory.
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        // Run planning_phase which should create three task files.
        planning_phase(base).expect("planning_phase should succeed");
        // Verify task_03.md exists and contains the correct title.
        let task_path = base.join("session_plan").join("task_03.md");
        assert!(task_path.is_file(), "task_03.md should exist");
        let content = fs::read_to_string(&task_path).expect("read task file");
        assert!(
            content.contains("Placeholder Task 3"),
            "task file should contain Placeholder Task 3 title"
        );
    }
}
