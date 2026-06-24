use bevy::ecs::resource::Resource;

#[derive(Resource)]
pub struct ReplOutputChannel {
    pub sender: crossbeam_channel::Sender<String>,
    pub receiver: crossbeam_channel::Receiver<String>,
}

impl Default for ReplOutputChannel {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self { sender, receiver }
    }
}

