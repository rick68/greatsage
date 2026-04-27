// Test specifically for Placeholder Task 3 generation

#[cfg(test)]
mod tests {
    use {std::fs, tempfile::TempDir};

    #[test]
    fn test_placeholder_task_3_content() {
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        crate::evolve::planning_phase(base).expect("planning_phase should succeed");
        let task_path = base.join("session_plan").join("task_03.md");
        assert!(task_path.is_file(), "task_03.md should exist");
        let content = fs::read_to_string(&task_path).expect("read task_03.md");
        assert!(
            content.contains("Address"),
            "Content should contain title derived from assessment"
        );
        assert!(
            content.contains("Files: none"),
            "Files line should be present"
        );
        assert!(
            content.contains("Issue: none"),
            "Issue line should be present"
        );
    }
}
