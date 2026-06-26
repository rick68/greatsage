//! Async agent-op result channel (tokio task → main-thread stdout drain).

use bevy::ecs::resource::Resource;

#[derive(Resource)]
pub(super) struct ReplOutputChannel {
    pub sender: crossbeam_channel::Sender<String>,
    pub receiver: crossbeam_channel::Receiver<String>,
}

impl Default for ReplOutputChannel {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self { sender, receiver }
    }
}
