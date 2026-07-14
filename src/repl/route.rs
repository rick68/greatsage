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
    Context,
    Init,
    Remember,
    Memories,
    Forget,
    Hooks,
    History,
    Search,
    Mark,
    Jump,
    Marks,
    Export,
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

    pub(super) const fn is_session_nav(self) -> bool {
        matches!(
            self,
            Self::History | Self::Search | Self::Mark | Self::Jump | Self::Marks | Self::Export
        )
    }

    pub(super) const fn is_lifecycle(self) -> bool {
        matches!(
            self,
            Self::Quit | Self::Exit | Self::Clear | Self::ClearForce
        )
    }

    pub(super) const fn is_info(self) -> bool {
        matches!(
            self,
            Self::Status | Self::Tokens | Self::Cost | Self::Context
        )
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
        "/retry" => CommandRoute::Retry,
        "/status" => CommandRoute::Status,
        "/tokens" => CommandRoute::Tokens,
        "/cost" => CommandRoute::Cost,
        "/hooks" => CommandRoute::Hooks,
        "/history" => CommandRoute::History,
        "/search" => CommandRoute::Search,
        "/mark" => CommandRoute::Mark,
        "/jump" => CommandRoute::Jump,
        "/marks" => CommandRoute::Marks,
        "/export" => CommandRoute::Export,
        // Project
        "/context" => CommandRoute::Context,
        "/init" => CommandRoute::Init,
        "/remember" => CommandRoute::Remember,
        "/memories" => CommandRoute::Memories,
        "/forget" => CommandRoute::Forget,
        // AI
        "/model" => CommandRoute::Model,
        "/provider" => CommandRoute::Provider,
        // otherwise
        _ if cmd.starts_with('/') => CommandRoute::UnknownSlash,
        _ => CommandRoute::NotSlash,
    }
}
