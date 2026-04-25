// Test specifically for Placeholder Task 2 generation

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    #[test]
    fn test_placeholder_task_2_content() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"));
        // Clean any existing plan dir to ensure fresh generation
        let _ = fs::remove_dir_all(base.join("session_plan"));
        // Run planning_phase to generate tasks
        crate::evolve::planning_phase(base).expect("planning_phase should succeed");
        let task_path = base.join("session_plan").join("task_02.md");
        assert!(task_path.is_file(), "task_02.md should exist");
        let content = fs::read_to_string(&task_path).expect("read task_02.md");
        assert!(
            content.contains("Placeholder Task 2"),
            "Content should contain title for task 2"
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
