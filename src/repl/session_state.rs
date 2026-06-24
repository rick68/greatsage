use bevy::ecs::resource::Resource;

#[derive(Resource, Default)]
pub struct ReplSessionState {
    pub last_user_prompt: Option<String>,
}