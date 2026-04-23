// Library entry point for the `greatsage` crate.
// Expose modules needed for library compilation and tests.

use bevy::prelude::Resource;

#[derive(Clone, Debug, Default, Resource)]
pub struct Args {
    // Skills directory list, matching the CLI Args in main.rs.
    pub skills: Option<Vec<std::path::PathBuf>>,
}

pub mod agents;
pub mod git;
pub mod tokio;
pub mod tui;
