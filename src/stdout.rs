use {
    crate::cli::{Cli, Command},
    bevy::{
        app::{App, PostUpdate},
        ecs::{
            change_detection::Res,
            message::{Message, MessageReader},
        },
    },
    colored::{ColoredString, Colorize},
    std::io::{self, Write},
};

/// REPL prompt prefix on a fresh line (`\n> `).
pub fn prompt_symbol() -> String {
    <&str as Colorize>::bold("\n> ").green().to_string()
}

/// REPL prompt prefix on the active input line (`> `).
pub fn prompt_symbol_inline() -> String {
    <&str as Colorize>::bold("> ").green().to_string()
}

/// BRP / external inject: show a submitted user prompt like REPL Enter (no channel send).
#[derive(Debug, Message, Clone)]
pub struct ExternPromptSubmitted(pub String);

#[derive(Message)]
pub struct StdoutMessage(String);

impl StdoutMessage {
    pub fn newline() -> Self {
        Self(String::from("\n"))
    }

    pub fn clear_line_from_cursor_to_end() -> Self {
        Self(String::from("\x1b[K"))
    }

    pub fn move_cursor_left() -> Self {
        Self(String::from("\x1b[1D"))
    }

    pub fn move_cursor_right() -> Self {
        Self(String::from("\x1b[1C"))
    }
}

impl From<&str> for StdoutMessage {
    fn from(value: &str) -> Self {
        Self(String::from(value))
    }
}

impl From<&mut str> for StdoutMessage {
    fn from(value: &mut str) -> Self {
        Self(String::from(value))
    }
}

impl From<String> for StdoutMessage {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&String> for StdoutMessage {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

impl From<ColoredString> for StdoutMessage {
    fn from(value: ColoredString) -> Self {
        Self(value.to_string())
    }
}

/// `greatsage tui` owns the terminal via bevy_ratatui alternate screen.
/// Writing REPL/agent stream bytes to stdout would scramble the full-screen UI.
fn stdout_print_enabled(cli: &Cli) -> bool {
    !matches!(cli.command, Some(Command::Tui))
}

fn print_stdout_message(cli: Res<Cli>, mut messages: MessageReader<StdoutMessage>) {
    if messages.is_empty() {
        return;
    }

    if !stdout_print_enabled(&cli) {
        // Drain so the buffer does not grow forever; do not touch the TTY.
        for _ in messages.read() {}
        return;
    }

    let mut lock = io::stdout().lock();

    for StdoutMessage(text) in messages.read() {
        let _ = lock.write(text.replace('\n', "\r\n").as_bytes());
    }

    let _ = lock.flush();
}

pub(crate) fn stdout_plugin(app: &mut App) {
    app.add_message::<StdoutMessage>()
        .add_message::<ExternPromptSubmitted>()
        .add_systems(PostUpdate, print_stdout_message);
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn tui_disables_stdout_print() {
        let mut cli = Cli::parse_from(["greatsage", "tui"]);
        assert!(!stdout_print_enabled(&cli));
        cli.command = None;
        assert!(stdout_print_enabled(&cli));
    }
}
