#[allow(clippy::module_inception)]
#[cfg(test)]
mod tests {
    use {
        crate::tui::{TuiMain, tui_main::handle_slash_command},
        std::env,
        tempfile::tempdir,
    };

    #[test]
    fn test_git_stage_error_in_non_git_dir() {
        let dir = tempdir().expect("failed to create temp dir");
        let original = env::current_dir().expect("could not get cwd");
        () = env::set_current_dir(dir.path()).expect("could not cd into temp dir");

        let mut tui = TuiMain::default();
        () = tui.set_input_public("/git stage".to_string());
        let lines = handle_slash_command("/git stage");
        () = tui.push_history_public("/git stage");
        for line in lines.clone() {
            tui.push_line(line);
        }
        () = tui.clear_input_public();
        () = tui.scroll_to_bottom();

        assert!(!lines.is_empty(), "expected at least one output line");
        let text = lines[0].to_string();
        assert!(
            text.contains("❌ /git stage failed"),
            "expected error indicator, got: {text}"
        );
        assert!(tui.input_is_empty(), "input not cleared");
        assert_eq!(tui.last_history(), Some("/git stage"));

        () = env::set_current_dir(original).expect("could not restore cwd");
    }
}
