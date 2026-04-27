// Test that executing a task titled "Address none" does not create any marker files and succeeds.

#[cfg(test)]
mod tests {
    use {greatsage::evolve::execute_tasks, std::fs, tempfile::TempDir};

    #[test]
    fn address_none_is_noop() {
        // Setup temporary base directory with session_plan and a task file.
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        let plan_dir = base.join("session_plan");
        fs::create_dir_all(&plan_dir).expect("create session_plan dir");
        let task_path = plan_dir.join("task_03.md");
        let task_content = "Title: Address none\nDetails: none\n";
        fs::write(&task_path, task_content).expect("write task file");

        // Execute tasks phase.
        execute_tasks(base).expect("execute_tasks should succeed");

        // Verify that no placeholder file was created for 'none'.
        let placeholder_dir = base.join(".greatsage");
        // The directory may exist due to log file, but there should be no placeholder files.
        if placeholder_dir.is_dir() {
            for entry in fs::read_dir(&placeholder_dir).expect("read .greatsage dir") {
                let entry = entry.expect("dir entry");
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                // Any file matching placeholder*.txt should not exist.
                assert!(
                    !name_str.starts_with("placeholder"),
                    "Unexpected placeholder file {} created",
                    name_str
                );
            }
        }
    }
}
