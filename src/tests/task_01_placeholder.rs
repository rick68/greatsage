// Test specifically for Placeholder Task 1 generation

#[cfg(test)]
mod tests {
    use {crate::evolve, std::fs, tempfile::TempDir};

    #[test]
    fn test_placeholder_task_1_content() {
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        () = evolve::planning_phase(base).expect("planning_phase should succeed");
        let task_path = base.join("session_plan").join("task_01.md");
        assert!(task_path.is_file(), "task_01.md should exist");
        let content = fs::read_to_string(&task_path).expect("read task_01.md");
        assert!(
            content.contains("Placeholder Task 1"),
            "Content should contain title for task 1"
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
