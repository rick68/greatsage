use {
    super::{
        components::{
            ActiveToolCall, SessionId, SessionIngestState, SessionRuntimeStatus, SessionSeq,
            ToolCallRecord, TurnEntity, TurnSummary,
        },
        resources::SessionManager,
    },
    crate::{
        agents::CodingAgentEvent,
        utils::{now_ms, truncate},
    },
    bevy::ecs::{
        change_detection::Res,
        entity::Entity,
        hierarchy::ChildOf,
        message::MessageReader,
        system::{Commands, Query},
    },
    yoagent::types::{AgentEvent, AgentMessage, Message, Usage},
};

fn tool_summary(
    tool_name: impl AsRef<str> + ToOwned<Owned = String>,
    args: &serde_json::Value,
) -> String {
    match tool_name.as_ref() {
        "bash" => {
            let cmd = args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("...");
            format!("$ {}", truncate(cmd, 80))
        }
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("read {path}")
        }
        "write_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("write {path}")
        }
        "edit_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("edit {path}")
        }
        "list_files" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("ls {path}")
        }
        "search" => {
            let pat = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            format!("search '{}'", truncate(pat, 60))
        }
        _ => tool_name.to_owned(),
    }
}

fn usage_from_message(message: &AgentMessage) -> Option<&Usage> {
    match message {
        AgentMessage::Llm(Message::Assistant { usage, .. }) => Some(usage),
        _ => None,
    }
}

fn usage_from_agent_end(messages: &[AgentMessage]) -> Option<Usage> {
    if let Some(msg) = messages.last()
        && let Some(usage) = usage_from_message(msg)
    {
        Some(usage.clone())
    } else {
        None
    }
}

fn next_seq(root: Entity, session_seq: &mut Query<&mut SessionSeq>) -> u64 {
    session_seq
        .get_mut(root)
        .map(|mut seq| seq.bump())
        .unwrap_or(0)
}

fn turn_summary_from_usage(seq: u64, usage: &Usage) -> TurnSummary {
    TurnSummary {
        seq,
        input: usage.input,
        output: usage.output,
        cache_read: usage.cache_read,
        cache_write: usage.cache_write,
        ended_at_ms: now_ms(),
    }
}

pub(crate) fn ingest_agent_events(
    mut messages: MessageReader<CodingAgentEvent>,
    mut commands: Commands,
    session_manager: Res<SessionManager>,
    mut session_roots: Query<(
        Entity,
        &SessionId,
        &mut SessionIngestState,
        &mut SessionRuntimeStatus,
    )>,
    mut session_seq: Query<&mut SessionSeq>,
    mut tool_records: Query<&mut ToolCallRecord>,
) {
    for msg in messages.read() {
        let session_id = msg.session_id;
        let Some(root) = session_manager.root_entity(session_id) else {
            continue;
        };

        let Ok((_, _, mut ingest_state, mut runtime_status)) = session_roots.get_mut(root) else {
            continue;
        };

        match &msg.event {
            AgentEvent::AgentStart => {
                () = runtime_status.set_processing();
                () = ingest_state.set_turn_recorded(false);
            }
            AgentEvent::AgentEnd { messages } => {
                runtime_status.set_idle();
                if !ingest_state.turn_recorded()
                    && let Some(usage) = usage_from_agent_end(messages)
                {
                    let seq = next_seq(root, &mut session_seq);
                    let turn = commands
                        .spawn((
                            session_id,
                            ChildOf(root),
                            TurnEntity,
                            turn_summary_from_usage(seq, &usage),
                        ))
                        .id();
                    () = ingest_state.set_current_turn(Some(turn));
                    () = ingest_state.set_turn_recorded(true);
                }
                () = ingest_state.clear_turn();
            }
            AgentEvent::TurnStart => {
                let turn = commands.spawn((session_id, ChildOf(root), TurnEntity)).id();
                () = ingest_state.set_current_turn(Some(turn));
            }
            AgentEvent::TurnEnd { message, .. } => {
                if let Some(usage) = usage_from_message(message) {
                    let seq = next_seq(root, &mut session_seq);
                    let summary = turn_summary_from_usage(seq, usage);
                    if let Some(turn) = ingest_state.take_current_turn() {
                        commands.entity(turn).insert(summary);
                    } else {
                        commands.spawn((session_id, ChildOf(root), TurnEntity, summary));
                    }
                    () = ingest_state.set_turn_recorded(true);
                }
                () = ingest_state.finish_turn();
            }
            AgentEvent::ToolExecutionStart {
                tool_call_id,
                tool_name,
                args,
            } => {
                let parent = ingest_state.current_turn().unwrap_or(root);
                let seq = next_seq(root, &mut session_seq);
                let entity = commands
                    .spawn((
                        session_id,
                        ChildOf(parent),
                        ToolCallRecord {
                            seq,
                            tool_call_id: tool_call_id.clone(),
                            tool_name: tool_name.clone(),
                            summary: tool_summary(tool_name.to_owned(), args),
                            started_at_ms: now_ms(),
                            ended_at_ms: None,
                            is_error: false,
                        },
                        ActiveToolCall,
                    ))
                    .id();
                () = ingest_state.track_tool(tool_call_id.clone(), entity);
            }
            AgentEvent::ToolExecutionEnd {
                tool_call_id,
                is_error,
                ..
            } => {
                if let Some(entity) = ingest_state.tool_entity(tool_call_id) {
                    if let Ok(mut record) = tool_records.get_mut(entity) {
                        record.ended_at_ms = Some(now_ms());
                        record.is_error = *is_error;
                    }
                    commands.entity(entity).remove::<ActiveToolCall>();
                    () = ingest_state.remove_tool(tool_call_id);
                }
            }
            _ => {}
        }
    }
}
