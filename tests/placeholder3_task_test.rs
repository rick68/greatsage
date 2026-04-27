// Test that planning_phase creates Placeholder Task 3 with correct title.

#[cfg(test)]
mod tests {
    use {greatsage::evolve::planning_phase, std::fs, tempfile::TempDir};

    #[test]
    fn planning_creates_placeholder_task_3() {
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        planning_phase(base).expect("planning_phase should succeed");
        let task_path = base.join("session_plan").join("task_03.md");
        assert!(task_path.is_file(), "task_03.md should exist");
        let content = fs::read_to_string(&task_path).expect("read task file");
        assert!(
            content.contains("Placeholder Task 3"),
            "task file should contain correct title"
        );
    }
}
