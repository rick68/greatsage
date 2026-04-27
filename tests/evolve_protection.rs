// Integration tests for protected path detection and enforcement in evolve.rs

// Added tests for absolute paths, symlinks, and hidden directories.

#[cfg(test)]
mod tests {
    use {
        greatsage::evolve::{execute_tasks, is_protected_path, planning_phase},
        std::{fs, io::Write, os, path::Path},
        tempfile::TempDir,
    };

    #[test]
    fn protects_known_paths() {
        let protected = [".github/workflows", "IDENTITY.md", "scripts", "skills"];
        for p in protected.iter() {
            let path = Path::new(p);
            assert!(is_protected_path(path), "{p} should be protected");
        }
    }

    #[test]
    fn does_not_protect_similar_names() {
        let non_protected = [
            "script",
            "scripts_backup",
            "skillful",
            "my/.github/workflows_extra",
        ];
        for p in non_protected.iter() {
            let path = Path::new(p);
            assert!(!is_protected_path(path), "{p} should NOT be protected");
        }
    }

    #[test]
    fn absolute_paths_are_protected() {
        // Use absolute path to a protected location.
        let cwd = std::env::current_dir().expect("current dir");
        let abs_path = cwd.join("scripts").join("util.sh");
        assert!(
            is_protected_path(&abs_path),
            "absolute path to protected location should be protected"
        );
    }

    #[test]
    fn hidden_directories_are_not_protected() {
        let hidden = [".secret/file.txt", ".hidden_dir/sub/file.rs"];
        for p in hidden.iter() {
            let path = Path::new(p);
            assert!(
                !is_protected_path(path),
                "{p} should NOT be protected (hidden directory)"
            );
        }
    }

    #[test]
    fn symlink_to_protected_path_is_detected() {
        // Create a temporary directory with a protected subdirectory and a symlink to it.
        let tmp = TempDir::new().expect("temp dir");
        let protected_dir = tmp.path().join("scripts");
        () = fs::create_dir_all(&protected_dir).expect("create protected dir");
        // Create a symlink named "link_scripts" pointing to the protected directory.
        cfg_if::cfg_if! {
            if #[cfg(unix)] {
                () = os::unix::fs::symlink(&protected_dir, tmp.path()
                    .join("link_scripts"))
                    .expect("create symlink");
            } else if #[cfg(windows)] {
                () = os::windows::fs::symlink_dir(&protected_dir, tmp.path()
                    .join("link_scripts"))
                    .expect("create symlink");
            }
        }
        // Resolve the symlink to its canonical path and test.
        let symlink_path = tmp.path().join("link_scripts");
        let canonical = std::fs::canonicalize(&symlink_path).expect("canonicalize symlink");
        assert!(
            is_protected_path(&canonical),
            "symlink to protected path should be considered protected"
        );
    }

    #[test]
    fn planning_phase_fails_on_protected_base() {
        // Create a temporary directory that mimics a protected location.
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path().join(".github").join("workflows");
        () = fs::create_dir_all(&base).expect("create protected dir");
        // Attempt planning_phase with the protected base directory.
        let result = planning_phase(&base);
        assert!(
            result.is_err(),
            "planning_phase should abort on protected path"
        );
    }

    #[test]
    fn execute_tasks_fails_on_protected_task_file() {
        // Set up a temporary base directory.
        let tmp = TempDir::new().expect("create temp dir");
        let base = tmp.path();
        // Create session_plan directory.
        let plan_dir = base.join("session_plan");
        () = fs::create_dir_all(&plan_dir).expect("create session_plan");
        // Create a protected task file inside a protected path.
        let protected_dir = base.join("scripts");
        () = fs::create_dir_all(&protected_dir).expect("create protected dir");
        let task_path = protected_dir.join("task_01.md");
        let mut file = fs::File::create(&task_path).expect("create task file");
        writeln!(file, "Title: Bad Task").expect("write task");
        // Run execute_tasks; it should detect the protected task file and error.
        let result = execute_tasks(base);
        assert!(
            result.is_err(),
            "execute_tasks should abort on protected task file"
        );
    }
}
