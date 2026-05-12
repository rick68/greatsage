use {
    bevy::{
        app::{App, PostUpdate},
        ecs::message::{Message, MessageReader},
    },
    colored::ColoredString,
    std::io::{self, Write},
};

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

fn print_stdout_message(mut messages: MessageReader<StdoutMessage>) {
    if !messages.is_empty() {
        let mut lock = io::stdout().lock();

        for StdoutMessage(text) in messages.read() {
            let _ = lock.write(text.replace('\n', "\r\n").as_bytes());
        }

        let _ = lock.flush();
    }
}

pub(crate) fn stdout_plugin(app: &mut App) {
    app.add_message::<StdoutMessage>()
        .add_systems(PostUpdate, print_stdout_message);
}
