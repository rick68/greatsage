use crate::tui::TuiMain;
use crate::tui::tui_main::handle_git_command;
use std::env;
use tempfile::tempdir;

#[test]
fn test_git_stage_error_handling() {
    // Create a temporary directory without a git repository to force an error.
    let dir = tempdir().expect("failed to create temp dir");
    let original = env::current_dir().expect("could not get cwd");
    env::set_current_dir(dir.path()).expect("could not cd into temp dir");

    // Initialize TuiMain and simulate entering "git stage".
    let mut tui = TuiMain::default();
    tui.set_input_public("git stage".to_string());
    // Handle the git command directly (error expected).
    let line = handle_git_command(&mut tui, "git stage");
    // Record history and output as the REPL would do.
    tui.push_history_public("git stage");
    tui.output.push(line.clone());
    // Clear input and adjust scroll.
    tui.clear_input_public();
    tui.scroll_to_bottom();

    // Verify the output line contains the error indicator.
    let text = line.to_string();
    assert!(
        text.contains("❌ git stage failed"),
        "expected error line, got: {}",
        text
    );
    // Input should be cleared.
    assert!(tui.input_is_empty(), "input not cleared");
    // History should contain the command.
    assert_eq!(tui.last_history(), Some("git stage"));

    // Restore original cwd.
    env::set_current_dir(original).expect("could not restore cwd");
}
