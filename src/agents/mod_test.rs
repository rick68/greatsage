use super::*;
use pretty_assertions::assert_eq;
use std::env;
use std::fs;
use tempfile::{TempDir, tempdir};

/// Helper to initialize a git repository in the given directory.
fn init_git_repo(dir: &TempDir) {
    // Initialize repository
    let _ = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("git init failed");
    // Set dummy user config to allow commits in test environment
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir.path())
        .output()
        .expect("git config email failed");
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir.path())
        .output()
        .expect("git config name failed");
}

#[test]
fn git_stage_all_on_clean_repo() {
    // Save current dir to restore later
    let original_dir = env::current_dir().expect("Failed to get current dir");
    let temp_dir = tempdir().expect("Failed to create temp dir");
    init_git_repo(&temp_dir);
    // Change to temp repo directory
    env::set_current_dir(temp_dir.path()).expect("Failed to set current dir");

    // No files added, stage_all should succeed (git add . does nothing but exits 0)
    let result = crate::git::stage_all();
    assert_eq!(result, Ok(()));

    // Restore original cwd
    env::set_current_dir(original_dir).expect("Failed to restore cwd");
}

#[test]
fn git_commit_without_changes_returns_error() {
    let original_dir = env::current_dir().expect("Failed to get current dir");
    let temp_dir = tempdir().expect("Failed to create temp dir");
    init_git_repo(&temp_dir);
    env::set_current_dir(temp_dir.path()).expect("Failed to set current dir");

    // Ensure there is at least one commit to allow a commit operation; create a file and commit it.
    let file_path = temp_dir.path().join("readme.txt");
    fs::write(&file_path, b"initial").expect("write failed");
    // Stage and commit initial file to have a repo with a commit.
    crate::git::stage_all().expect("stage failed");
    crate::git::commit("initial commit").expect("initial commit failed");

    // Now repo is clean; attempt to commit again with no changes.
    let result = crate::git::commit("empty commit");
    assert!(
        result.is_err(),
        "Expected error when committing with no changes"
    );
    // The error should mention "nothing to commit" or similar.
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.to_lowercase().contains("nothing to commit")
            || err_msg.to_lowercase().contains("no changes"),
        "Unexpected error message: {}",
        err_msg
    );

    env::set_current_dir(original_dir).expect("Failed to restore cwd");
}
