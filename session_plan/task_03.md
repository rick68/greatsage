Title: Add unit test for Git commit helper
Files: tests/git_commit_test.rs
Issue: none

Create a test `fn test_commit_changes_fails_without_repo()` that creates a temporary directory without a Git repository, sets the current directory to it, and calls `commit_changes("test")`. The function should return an error because `git commit` will fail. Assert that the error is Err and contains appropriate message. This ensures the new helper behaves correctly in edge cases.
