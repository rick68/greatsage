use {
    super::{
        components::{
            ActiveToolCall, ContentBlock, ContentBlockEntity, ContentProvenance, IndexedContent,
            SessionId, SessionIngestState, SessionRuntimeStatus, SessionSeq, ToolCallRecord,
            TurnEntity, TurnSummary,
        },
        content::{
            content_kind_label, content_primary_text, indexed_content_from_assistant_message,
            indexed_content_from_streamed_assistant, indexed_content_from_tool_start,
            indexed_content_from_turn_tool_result, indexed_content_from_user_message,
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
    std::slice,
    yoagent::types::{AgentEvent, AgentMessage, Content, Message, StreamDelta, Usage},
};

fn assistant_blocks_for_projection(
    message: Option<&AgentMessage>,
    streamed_assistant_text: &str,
) -> Vec<IndexedContent> {
    let mut blocks = message
        .map(indexed_content_from_assistant_message)
        .unwrap_or_default();
    let has_text_block = blocks
        .iter()
        .any(|block| matches!(block.content, Content::Text { .. }));
    let trimmed = streamed_assistant_text.trim();
    if !has_text_block && !trimmed.is_empty() {
        let block_index = blocks.len() as u32;
        let mut streamed = indexed_content_from_streamed_assistant(trimmed, now_ms());
        if let Some(block) = streamed.first_mut() {
            block.block_index = block_index;
        }
        () = blocks.extend(streamed);
    }
    blocks
}

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

fn usage_from_agent_end(messages: &[AgentMessage]) -> Option<&Usage> {
    messages
        .iter()
        .rev()
        .find_map(|message| usage_from_message(message))
}

fn assistant_message_from_agent_end(messages: &[AgentMessage]) -> Option<&AgentMessage> {
    messages
        .iter()
        .rev()
        .find(|message| usage_from_message(message).is_some())
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
        input_tokens: usage.input,
        output_tokens: usage.output,
        cache_read_tokens: usage.cache_read,
        cache_write_tokens: usage.cache_write,
        ended_at_ms: now_ms(),
    }
}

fn provenance_kind(provenance: &ContentProvenance) -> &'static str {
    match provenance {
        ContentProvenance::UserMessage { .. } => "userMessage",
        ContentProvenance::AssistantMessage { .. } => "assistantMessage",
        ContentProvenance::ToolExecutionStart { .. } => "toolExecutionStart",
        ContentProvenance::TurnToolResult { .. } => "turnToolResult",
    }
}

fn provenance_tool_name(provenance: &ContentProvenance) -> Option<String> {
    match provenance {
        ContentProvenance::ToolExecutionStart { tool_name, .. }
        | ContentProvenance::TurnToolResult { tool_name, .. } => Some(tool_name.clone()),
        _ => None,
    }
}

fn provenance_is_error(provenance: &ContentProvenance) -> Option<bool> {
    match provenance {
        ContentProvenance::TurnToolResult { is_error, .. } => Some(*is_error),
        _ => None,
    }
}

fn content_block_from_indexed(
    seq: u64,
    recorded_at_ms: u64,
    indexed: &IndexedContent,
) -> ContentBlock {
    ContentBlock {
        seq,
        block_index: indexed.block_index,
        recorded_at_ms,
        source_timestamp_ms: indexed.provenance.source_timestamp_ms(),
        provenance_kind: provenance_kind(&indexed.provenance).to_string(),
        content_kind: content_kind_label(&indexed.content).to_string(),
        content_text: content_primary_text(&indexed.content),
        content_json: serde_json::to_string(&indexed.content).unwrap_or_default(),
        tool_call_id: indexed.provenance.tool_call_id().map(str::to_string),
        tool_name: provenance_tool_name(&indexed.provenance),
        is_error: provenance_is_error(&indexed.provenance),
    }
}

fn spawn_indexed_blocks(
    commands: &mut Commands,
    session_id: SessionId,
    parent: Entity,
    root: Entity,
    session_seq: &mut Query<&mut SessionSeq>,
    blocks: &[IndexedContent],
) {
    let recorded_at_ms = now_ms();
    for indexed in blocks {
        let seq = next_seq(root, session_seq);
        commands.spawn((
            session_id,
            ChildOf(parent),
            ContentBlockEntity,
            content_block_from_indexed(seq, recorded_at_ms, indexed),
        ));
    }
}

fn project_turn_content(
    commands: &mut Commands,
    session_id: SessionId,
    turn: Entity,
    root: Entity,
    session_seq: &mut Query<&mut SessionSeq>,
    user_content: &[IndexedContent],
    message: Option<&AgentMessage>,
    streamed_assistant_text: &str,
) {
    spawn_indexed_blocks(commands, session_id, turn, root, session_seq, user_content);
    let assistant = assistant_blocks_for_projection(message, streamed_assistant_text);
    spawn_indexed_blocks(commands, session_id, turn, root, session_seq, &assistant);
}

fn project_turn_tool_results(
    commands: &mut Commands,
    ingest_state: &mut SessionIngestState,
    session_id: SessionId,
    root: Entity,
    session_seq: &mut Query<&mut SessionSeq>,
    tool_results: &[Message],
) {
    for message in tool_results {
        let Message::ToolResult { tool_call_id, .. } = message else {
            continue;
        };
        if ingest_state.turn_tool_result_already_projected(tool_call_id) {
            continue;
        }
        let Some(tool_entity) = ingest_state.tool_entity(tool_call_id) else {
            continue;
        };
        let blocks = indexed_content_from_turn_tool_result(message);
        spawn_indexed_blocks(
            commands,
            session_id,
            tool_entity,
            root,
            session_seq,
            &blocks,
        );
        ingest_state.mark_turn_tool_result_projected(tool_call_id);
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
                () = ingest_state.reset_invocation();
            }
            AgentEvent::AgentEnd { messages } => {
                runtime_status.set_idle();
                if !ingest_state.turn_recorded()
                    && let Some(usage) = usage_from_agent_end(messages)
                {
                    let seq = next_seq(root, &mut session_seq);
                    let user_content = ingest_state.take_user_content_for_turn();
                    let streamed_assistant_text = ingest_state.take_pending_assistant_text();
                    let message = assistant_message_from_agent_end(messages);
                    let turn = commands
                        .spawn((
                            session_id,
                            ChildOf(root),
                            TurnEntity,
                            turn_summary_from_usage(seq, usage),
                        ))
                        .id();
                    project_turn_content(
                        &mut commands,
                        session_id,
                        turn,
                        root,
                        &mut session_seq,
                        &user_content,
                        message,
                        &streamed_assistant_text,
                    );
                    () = ingest_state.set_current_turn(Some(turn));
                    () = ingest_state.set_turn_recorded(true);
                }
                () = ingest_state.clear_turn();
            }
            AgentEvent::TurnStart => {
                let turn = commands.spawn((session_id, ChildOf(root), TurnEntity)).id();
                () = ingest_state.set_current_turn(Some(turn));
            }
            AgentEvent::MessageEnd { message } => {
                let blocks = indexed_content_from_user_message(message);
                // Assistant `MessageEnd` also flows through this arm; ignore non-user
                // messages so we do not clear a prompt captured earlier in the turn.
                if !blocks.is_empty() {
                    () = ingest_state.set_pending_user_content(blocks);
                }
            }
            AgentEvent::TurnEnd {
                message,
                tool_results,
            } => {
                if let Some(usage) = usage_from_message(message) {
                    let seq = next_seq(root, &mut session_seq);
                    let user_content = ingest_state.take_user_content_for_turn();
                    let streamed_assistant_text = ingest_state.take_pending_assistant_text();
                    let summary = turn_summary_from_usage(seq, usage);
                    let turn = if let Some(turn) = ingest_state.take_current_turn() {
                        commands.entity(turn).insert(summary);
                        turn
                    } else {
                        commands
                            .spawn((session_id, ChildOf(root), TurnEntity, summary))
                            .id()
                    };
                    project_turn_content(
                        &mut commands,
                        session_id,
                        turn,
                        root,
                        &mut session_seq,
                        &user_content,
                        Some(message),
                        &streamed_assistant_text,
                    );
                    project_turn_tool_results(
                        &mut commands,
                        &mut ingest_state,
                        session_id,
                        root,
                        &mut session_seq,
                        tool_results,
                    );
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
                let argument = indexed_content_from_tool_start(tool_call_id, tool_name, args);
                () = spawn_indexed_blocks(
                    &mut commands,
                    session_id,
                    entity,
                    root,
                    &mut session_seq,
                    slice::from_ref(&argument),
                );
                () = ingest_state.track_tool(tool_call_id.clone(), entity);
            }
            AgentEvent::ToolExecutionEnd {
                tool_call_id,
                tool_name,
                is_error,
                result,
                ..
            } => {
                if let Some(entity) = ingest_state.tool_entity(tool_call_id) {
                    let ended_at_ms = now_ms();
                    if let Ok(mut record) = tool_records.get_mut(entity) {
                        record.ended_at_ms = Some(ended_at_ms);
                        record.is_error = *is_error;
                    }
                    let _ = (tool_name, result);
                    commands.entity(entity).remove::<ActiveToolCall>();
                }
            }
            AgentEvent::MessageUpdate {
                delta: StreamDelta::Text { delta },
                ..
            } => {
                () = ingest_state.append_assistant_text(delta);
            }
            _ => {}
        }
    }
}
