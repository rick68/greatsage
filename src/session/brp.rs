use {
    super::{
        components::{AgentRuntimeState, SessionId, SessionRuntimeStatus},
        resources::{FocusedSession, SessionManager},
    },
    crate::agents::CodingAgentPromptChannel,
    bevy::{
        ecs::{system::In, world::World},
        remote::{BrpError, BrpResult, RemotePlugin, error_codes},
    },
    serde_json::{Value, json},
};

fn session_error(code: i16, message: impl Into<String>) -> BrpError {
    BrpError {
        code,
        message: message.into(),
        data: None,
    }
}

fn resolve_session_id(world: &World, params: Option<&Value>) -> Result<SessionId, BrpError> {
    if let Some(params) = params
        && let Some(raw) = params.get("session_id")
    {
        let id = raw.as_u64().ok_or_else(|| {
            session_error(error_codes::INVALID_PARAMS, "session_id must be a u64")
        })?;
        return Ok(SessionId(id));
    }

    if let Some(focused) = world.get_resource::<FocusedSession>().and_then(|f| f.0) {
        return Ok(focused);
    }

    Ok(SessionId(0))
}

fn prompt_from_params(params: Option<Value>) -> Result<String, BrpError> {
    let params = params.ok_or_else(|| {
        session_error(
            error_codes::INVALID_PARAMS,
            "Missing prompt or text parameter",
        )
    })?;

    let prompt = params
        .get("prompt")
        .or_else(|| params.get("text"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            session_error(
                error_codes::INVALID_PARAMS,
                "Missing or empty prompt or text parameter",
            )
        })?;

    Ok(prompt)
}

pub fn session_send_prompt(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let prompt = prompt_from_params(params)?;

    let channel = world
        .get_resource::<CodingAgentPromptChannel>()
        .ok_or_else(|| {
            BrpError::resource_not_present("greatsage::agents::CodingAgentPromptChannel")
        })?;

    () = channel.send_prompt(prompt);
    Ok(json!({ "accepted": true }))
}

pub fn session_runtime_status(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let session_id = resolve_session_id(world, params.as_ref())?;
    let root = world
        .resource::<SessionManager>()
        .root_entity(session_id)
        .ok_or_else(|| {
            session_error(
                error_codes::ENTITY_NOT_FOUND,
                format!("Session {session_id:?} not found"),
            )
        })?;

    let status = world
        .get_entity(root)
        .ok()
        .and_then(|entity| entity.get::<SessionRuntimeStatus>())
        .ok_or_else(|| {
            BrpError::component_not_present(
                "greatsage::session::components::SessionRuntimeStatus",
                root,
            )
        })?;

    let state = match status.runtime_state() {
        AgentRuntimeState::Idle => "idle",
        AgentRuntimeState::Processing => "processing",
    };

    Ok(json!({
        "session_id": session_id.0,
        "state": state,
    }))
}

pub fn register_session_brp_methods(plugin: RemotePlugin) -> RemotePlugin {
    plugin
        .with_method("session.send_prompt", session_send_prompt)
        .with_method("session.runtime_status", session_runtime_status)
}
