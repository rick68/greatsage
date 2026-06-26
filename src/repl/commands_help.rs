//! Help command: /help, /help <cmd>.
//!
//! [`help`] is the sole entry — scroll up into [`super::help_data`] for callees.

use super::{
    dispatch::DispatchResult,
    help_data::{format_help_detail_parts, help_text_lines},
};

pub(super) fn help(args: &str) -> DispatchResult {
    let (output, detail) = if args.is_empty() {
        (Vec::new(), help_text_lines())
    } else {
        let (usage, detail) = format_help_detail_parts(args);
        let mut lines = vec![usage];
        if !detail.is_empty() {
            lines.push(String::new());
            lines.extend(detail);
        }
        (Vec::new(), lines)
    };
    DispatchResult::Handled {
        output,
        detail,
        redraw_prompt: true,
        reinstall: None,
    }
}
