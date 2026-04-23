// Minimal placeholder definitions for library compilation.
// The binary provides real implementations in other modules.

/// Placeholder for command‑line arguments used by the binary.
#[derive(Clone, Debug)]
pub struct Args;

/// Placeholder cancellation token used by the binary's Tokio runtime.
#[derive(Clone, Debug)]
pub struct AppCancelToken;

/// Placeholder TUI main structure used by the binary.
#[derive(Clone, Debug)]
pub struct TuiMain;
