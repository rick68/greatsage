use {
    git2::{IndexAddOption, Repository},
    std::process::Command,
};

/// Stage all changes in the current repository using `git2`.
/// Returns `Ok(())` on success or an `Err` containing a description.
#[allow(dead_code)]
pub fn stage_all() -> Result<(), git2::Error> {
    let repo = Repository::discover(".")?;
    let mut index = repo.index()?;
    () = index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
    () = index.write()?;
    Ok(())
}

/// Commit staged changes with the given message using `git2`.
/// Returns `Ok(())` on success or an `Err` containing a description.
#[allow(dead_code)]
pub fn commit(message: impl AsRef<str>) -> Result<(), git2::Error> {
    let repo = Repository::discover(".")?;
    let mut index = repo.index()?;
    let tree_id = index.write_tree()?;
    () = drop(index);
    let tree = repo.find_tree(tree_id)?;

    // Call repo.head() once; reuse for both the "nothing to commit" check and parent resolution.
    let head_result = repo.head();

    // If there are no changes compared to HEAD, return early.
    if let Ok(ref head_ref) = head_result
        && let Some(head_oid) = head_ref.target()
    {
        let head_commit = repo.find_commit(head_oid)?;
        let head_tree = head_commit.tree()?;
        let diff = repo.diff_tree_to_tree(Some(&head_tree), Some(&tree), None)?;
        if diff.deltas().len() == 0 {
            return Err(git2::Error::from_str("nothing to commit"));
        }
    }

    let sig = repo.signature()?;
    // Resolve the parent commit, distinguishing three cases:
    //   Err  → unborn repo, this is the initial commit (no parent)
    //   Ok + target Some(oid) → normal commit, use oid as parent
    //   Ok + target None → HEAD exists but has no direct OID, which is
    //                       a corrupted/unexpected state; return an error
    //                       rather than silently creating an orphan commit.
    let parents = match head_result {
        Err(_) => vec![],
        Ok(ref head_ref) => {
            let oid = head_ref
                .target()
                .ok_or(git2::Error::from_str("HEAD has no target OID"))?;
            vec![repo.find_commit(oid)?]
        }
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    _ = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        message.as_ref(),
        &tree,
        &parent_refs,
    )?;
    Ok(())
}

/// Create a revert commit that undoes the most recent commit using `git2`.
/// Applies the inverse changes and commits them in one step.
/// Returns `Ok(())` on success or an `Err` containing a description.
#[allow(dead_code)]
pub fn revert_last() -> Result<(), git2::Error> {
    let repo = Repository::discover(".")?;
    let head = repo.head()?;
    let target = repo.find_commit(
        head.target()
            .ok_or(git2::Error::from_str("HEAD has no target"))?,
    )?;
    // Apply inverse changes to the index and working directory.
    () = repo.revert(&target, None)?;
    // Create the revert commit from the staged result.
    let message = format!("Revert \"{}\"", target.summary().unwrap_or(""));
    commit(&message)
}

/// Error type for git operations used by `commit_and_tag`.
pub type GitError = Box<dyn std::error::Error + Send + Sync>;

/// Stage all changes, commit with a message based on the iteration number,
/// create an annotated tag `v{iteration}`, and optionally push to the remote.
///
/// Returns `Ok(())` on success or an `Err` containing a description.
pub fn commit_and_tag(iteration: u32, push: bool) -> Result<(), GitError> {
    // Stage all changes.
    () = stage_all().map_err(|e| Box::new(e) as GitError)?;
    // Commit with a message.
    let msg = format!("evolve iteration {iteration}");
    ()  = commit(&msg).map_err(|e| Box::new(e) as GitError)?;
    // Create annotated tag.
    let tag_name = format!("v{iteration}");
    let tag_msg = format!("evolve iteration {iteration}");
    let tag_status = Command::new("git")
        .args(["tag", "-a", &tag_name, "-m", &tag_msg])
        .output()?;
    if !tag_status.status.success() {
        let err = String::from_utf8_lossy(&tag_status.stderr).into_owned();
        return Err(Box::new(std::io::Error::other(err)));
    }
    if push {
        // Push commits.
        let push_status = Command::new("git").args(["push"]).output()?;
        if !push_status.status.success() {
            let err = String::from_utf8_lossy(&push_status.stderr).into_owned();
            return Err(Box::new(std::io::Error::other(err)));
        }
        // Push tags.
        let push_tags_status = Command::new("git").args(["push", "--tags"]).output()?;
        if !push_tags_status.status.success() {
            let err = String::from_utf8_lossy(&push_tags_status.stderr).into_owned();
            return Err(Box::new(std::io::Error::other(err)));
        }
    }
    Ok(())
}

/// Commit staged changes using the system `git` command.
/// Runs `git add -A && git commit -m "message"`.
/// Returns `Ok(())` on success, or an `Err` containing the command's stderr.
#[allow(dead_code)]
pub fn commit_changes(message: impl AsRef<str>) -> Result<(), Box<dyn std::error::Error>> {
    // Stage all changes.
    let add_output = Command::new("git").args(["add", "-A"]).output()?;
    if !add_output.status.success() {
        let err = String::from_utf8_lossy(&add_output.stderr).into_owned();
        return Err(err.into());
    }
    // Commit with the provided message.
    let commit_output = Command::new("git")
        .args(["commit", "-m", message.as_ref()])
        .output()?;
    if commit_output.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&commit_output.stderr).into_owned();
        Err(err.into())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        pretty_assertions::assert_eq,
        std::{env, fs, sync::Mutex},
        temp_env_vars::temp_env_vars,
        tempfile::{TempDir, tempdir},
    };

    // Serialise tests that mutate the process-global cwd via set_current_dir.
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    /// RAII guard that restores the working directory on drop (even on panic).
    struct CwdGuard(std::path::PathBuf);
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = env::set_current_dir(&self.0);
        }
    }

    /// Helper to initialize a git repository in the given directory using `git2`.
    fn init_git_repo(dir: &TempDir) {
        let repo = Repository::init(dir.path()).expect("git init failed");
        let mut cfg = repo
            .config()
            .expect("Failed to read repository configuration");
        () = cfg
            .set_str("user.email", "test@example.com")
            .expect("Failed to set user email in git config");
        () = cfg
            .set_str("user.name", "Test User")
            .expect("Failed to set user name in git config");
        let init_path = dir.path().join("init.txt");
        () = std::fs::write(&init_path, b"init").expect("Failed to write initial file");
        let mut index = repo.index().expect("Failed to get repository index");
        () = index
            .add_path(std::path::Path::new("init.txt"))
            .expect("Failed to add init file to index");
        () = index.write().expect("Failed to write index");
        let tree_id = index.write_tree().expect("Failed to write tree");
        let tree = repo.find_tree(tree_id).expect("Failed to find tree");
        () = drop(index);
        let sig = repo.signature().expect("Failed to create git signature");
        _ = repo
            .commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .expect("Failed to create initial commit");
    }

    #[test]
    #[temp_env_vars]
    fn git_stage_all_on_clean_repo() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = tempdir().expect("Failed to create temp dir");
        () = init_git_repo(&temp_dir);
        let _cwd = CwdGuard(env::current_dir().expect("Failed to get current dir"));
        () = env::set_current_dir(temp_dir.path()).expect("Failed to set current dir");
        let result = crate::git::stage_all();
        assert_eq!(result, Ok(()));
    }

    #[test]
    #[temp_env_vars]
    fn git_revert_last_restores_previous_state() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = tempdir().expect("Failed to create temp dir");
        () = init_git_repo(&temp_dir);
        let _cwd = CwdGuard(env::current_dir().expect("Failed to get current dir"));
        () = env::set_current_dir(temp_dir.path()).expect("Failed to set current dir");

        // Add a file and commit it.
        let file_path = temp_dir.path().join("added.txt");
        () = fs::write(&file_path, b"hello").expect("write failed");
        () = stage_all().expect("stage failed");
        () = commit("add file").expect("commit failed");
        assert!(file_path.exists());

        // Revert that commit — the file should disappear from the tree.
        () = revert_last().expect("revert failed");
        assert!(
            !file_path.exists(),
            "revert should have removed the file from the working tree"
        );
    }

    #[test]
    #[temp_env_vars]
    fn git_commit_without_changes_returns_error() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = tempdir().expect("Failed to create temp dir");
        () = init_git_repo(&temp_dir);
        let _cwd = CwdGuard(env::current_dir().expect("Failed to get current dir"));
        () = env::set_current_dir(temp_dir.path()).expect("Failed to set current dir");
        let file_path = temp_dir.path().join("readme.txt");
        () = fs::write(&file_path, b"initial").expect("write failed");
        () = stage_all().expect("stage failed");
        () = commit("initial commit").expect("initial commit failed");
        let result = crate::git::commit("empty commit");
        assert!(result.is_err());
    }

    #[test]
    #[temp_env_vars]
    fn commit_changes_successful() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = tempdir().expect("Failed to create temp dir");
        () = init_git_repo(&temp_dir);
        let _cwd = CwdGuard(env::current_dir().expect("Failed to get current dir"));
        () = env::set_current_dir(temp_dir.path()).expect("Failed to set current dir");
        // Create a new file.
        let file_path = temp_dir.path().join("new.txt");
        () = fs::write(&file_path, b"data").expect("write failed");
        // Use commit_changes.
        let result = commit_changes("add new file");
        assert!(result.is_ok());
        // Verify the commit exists.
        let log_output = Command::new("git")
            .args(["log", "--oneline"])
            .output()
            .expect("git log failed");
        let log_str = String::from_utf8_lossy(&log_output.stdout);
        assert!(log_str.contains("add new file"));
    }

    #[test]
    #[temp_env_vars]
    fn commit_changes_no_changes_error() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let temp_dir = tempdir().expect("Failed to create temp dir");
        () = init_git_repo(&temp_dir);
        let _cwd = CwdGuard(env::current_dir().expect("Failed to get current dir"));
        () = env::set_current_dir(temp_dir.path()).expect("Failed to set current dir");
        // No changes.
        let result = commit_changes("nothing to do");
        assert!(result.is_err());
    }
}
