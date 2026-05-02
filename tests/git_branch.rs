use greatsage::utils::current_git_branch;

use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn detects_branch_in_git_repo() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path();
    // init git repo
    Command::new("git")
        .args(["init", "-q", path.to_str().unwrap()])
        .output()
        .expect("git init");
    // configure user to allow commit
    Command::new("git")
        .args([
            "-C",
            path.to_str().unwrap(),
            "config",
            "user.email",
            "test@example.com",
        ])
        .output()
        .ok();
    Command::new("git")
        .args([
            "-C",
            path.to_str().unwrap(),
            "config",
            "user.name",
            "Test User",
        ])
        .output()
        .ok();
    // create a file and commit
    fs::write(path.join("README.md"), "test").unwrap();
    Command::new("git")
        .args(["-C", path.to_str().unwrap(), "add", "."])
        .output()
        .ok();
    Command::new("git")
        .args(["-C", path.to_str().unwrap(), "commit", "-m", "init", "-q"])
        .output()
        .ok();
    // Determine expected branch via git
    let expected = {
        let out = Command::new("git")
            .args([
                "-C",
                path.to_str().unwrap(),
                "rev-parse",
                "--abbrev-ref",
                "HEAD",
            ])
            .output()
            .expect("git rev-parse");
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    // Change cwd
    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(path).unwrap();
    let branch = current_git_branch();
    std::env::set_current_dir(orig).unwrap();
    assert_eq!(branch.as_deref(), Some(expected.as_str()));
}

#[test]
fn returns_none_outside_git_repo() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path();
    let orig = std::env::current_dir().unwrap();
    std::env::set_current_dir(path).unwrap();
    let branch = current_git_branch();
    std::env::set_current_dir(orig).unwrap();
    assert!(branch.is_none());
}
