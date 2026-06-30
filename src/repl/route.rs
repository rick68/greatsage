//! Pure slash-command routing (no side effects, no Bevy types).
//!
//! [`route_command`] maps the command token to a [`CommandRoute`]. Domain handlers
//! receive args separately via [`super::dispatch::command_name_and_args`].
//!
//! File entry last ([`route_command`]); each function directly above its callers.
//! Local callees sit immediately above their caller in source appearance order.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CommandRoute {
    Quit,
    Exit,
    Clear,
    ClearForce,
    Help,
    Provider,
    Model,
    Save,
    Load,
    Compact,
    Retry,
    Status,
    Tokens,
    Cost,
    UnknownSlash,
    NotSlash,
}

impl CommandRoute {
    pub(super) const fn is_session(self) -> bool {
        matches!(
            self,
            Self::Provider | Self::Model | Self::Save | Self::Load | Self::Compact | Self::Retry
        )
    }

    pub(super) const fn is_lifecycle(self) -> bool {
        matches!(
            self,
            Self::Quit | Self::Exit | Self::Clear | Self::ClearForce
        )
    }

    pub(super) const fn is_info(self) -> bool {
        matches!(self, Self::Status | Self::Tokens | Self::Cost)
    }
}

pub(super) fn route_command(cmd: &str) -> CommandRoute {
    match cmd {
        // session
        "/help" => CommandRoute::Help,
        "/quit" => CommandRoute::Quit,
        "/exit" => CommandRoute::Exit,
        "/clear" => CommandRoute::Clear,
        "/clear!" => CommandRoute::ClearForce,
        "/compact" => CommandRoute::Compact,
        "/save" => CommandRoute::Save,
        "/load" => CommandRoute::Load,
        "/retry" | "/rerty" => CommandRoute::Retry,
        // AI
        "/model" => CommandRoute::Model,
        "/provider" => CommandRoute::Provider,
        "/status" => CommandRoute::Status,
        "/tokens" => CommandRoute::Tokens,
        "/cost" => CommandRoute::Cost,
        // otherwise
        _ if cmd.starts_with('/') => CommandRoute::UnknownSlash,
        _ => CommandRoute::NotSlash,
    }
}
