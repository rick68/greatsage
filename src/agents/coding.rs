//! # Coding Agent — Event Handling
//!
//! ## Agent event flow
//! Events arrive via `CodingAgentEvent` messages on the Bevy main thread.
//! `handle_coding_agent_events` matches each `AgentEvent` and drives the TUI
//! through the `TuiMain` public API (never accessing `tui.blocks` directly).
//!
//! ## ThinkingBlock integration
//! When `StreamDelta::Thinking` arrives, `tui.begin_thinking()` opens a new
//! ThinkingBlock (spinner visible immediately).  Each subsequent delta calls
//! `tui.append_thinking(delta)`.  On the first `StreamDelta::Text` after a
//! thinking sequence, `tui.end_thinking(0)` seals the block (token count is
//! not available per-delta; the final tally comes from `AgentEnd`).
//!
//! ## StreamingText integration
//! On the first `StreamDelta::Text` delta, `tui.begin_streaming_text()` opens
//! a `StreamingText` block with a fixed header.  Each delta calls
//! `tui.update_streaming_text(rendered_lines)`, replacing the block in-place.
//! When a tool call interrupts the stream, `tui.finalize_streaming_text()`
//! converts the block to static `Lines` before tool output is appended.
//!
//! ## Coordinate mapping note
//! `tui_output_index` (previously used for flat-Vec truncation) is gone.
//! The `StreamingText` block owns its rendered lines and replaces them atomically.

use {
    crate::{
        agents::{
            AgentsCancelToken, LlmConfig, McpConfig, PermissionConfig, build_tools, retry_async,
        },
        config::AppConfig,
        tokio::AppCancelToken,
        tui::{RenderNeeded, TuiMain},
    },
    ansi_to_tui::IntoText as _,
    anyhow::anyhow,
    bevy::{
        app::{App, AppExit, PostUpdate, Startup, Update},
        ecs::{
            change_detection::{NonSendMut, Res, ResMut},
            message::{Message, MessageReader},
            resource::Resource,
            schedule::{
                IntoScheduleConfigs, SystemCondition,
                common_conditions::{not, resource_exists},
            },
            system::Commands,
            world::World,
        },
        prelude::{Deref, DerefMut},
        state::{
            app::AppExtStates,
            condition::in_state,
            state::{NextState, States},
        },
    },
    bevy_tokio_tasks::{MainThreadContext, TokioTasksRuntime},
    ratatui::{style::Stylize, text::Line},
    std::{
        fmt,
        io::{Error as IoError, Write, stdout},
        sync::Arc,
    },
    tokio::{sync::Mutex, task::JoinHandle},
    yoagent::{
        agent::Agent,
        context::ExecutionLimits,
        provider::{ModelConfig, openai_compat::OpenAiCompatProvider},
        skills::SkillSet,
        types::{AgentEvent, AgentMessage, StreamDelta, ThinkingLevel, Usage},
    },
};

const SYSTEM_PROMPT: &str = r#"You are a coding assistant working in the user's terminal.
You have access to the filesystem and shell. Be direct and concise.
When the user asks you to do something, do it — don't just explain how.
Use tools proactively: read files to understand context, run commands to verify your work.
After making changes, run tests or verify the result when appropriate."#;

#[derive(Deref, DerefMut, Resource)]
struct CodingAgent(Arc<Mutex<anyhow::Result<Agent>>>);

#[derive(Clone, Resource)]
pub struct CodingAgentPromptChannel {
    pub sender: crossbeam_channel::Sender<String>,
    pub receiver: crossbeam_channel::Receiver<String>,
}

impl Default for CodingAgentPromptChannel {
    fn default() -> Self {
        let (sender, receiver): (
            crossbeam_channel::Sender<String>,
            crossbeam_channel::Receiver<String>,
        ) = crossbeam_channel::unbounded::<String>();

        Self { sender, receiver }
    }
}

#[allow(clippy::too_many_arguments)]
fn setup(
    llm_config: Res<LlmConfig>,
    app_config: Res<AppConfig>,
    permission: Res<PermissionConfig>,
    mut tui: Option<NonSendMut<TuiMain>>,
    app_cancel: Res<AppCancelToken>,
    agents_cancel: Res<AgentsCancelToken>,
    mut commands: Commands,
    tokio_runtime: ResMut<TokioTasksRuntime>,
) {
    let runtime = app_config.runtime.clone();
    let mcp_config = McpConfig::from(runtime.mcp_servers.clone());

    () = commands.insert_resource::<McpConfig>(mcp_config.clone());

    if let Some(tui) = tui.as_mut() {
        () = tui.push_line(Line::from("🚀 Starting Interactive Coding Agent Session"));
        if !mcp_config.sse_transports.is_empty() || !mcp_config.stdio_transports.is_empty() {
            () = tui.push_line(Line::from("🔌 Connecting to MCP servers...").yellow());
            () = tui.scroll_to_bottom();
        }
    }

    let llm_config = llm_config.clone();
    let allowed_dir = permission.allowed_dir.clone();
    let app_cancel = app_cancel.clone();
    let agents_cancel = agents_cancel.clone();

    let _: JoinHandle<()> = tokio_runtime.spawn_background_task(move |mut ctx| async move {
        let mut model_config = ModelConfig::local(&llm_config.base_url, &llm_config.model);
        model_config.max_tokens = llm_config.max_tokens;
        model_config.context_window = llm_config.context_window;
        let thinking_level = match llm_config.thinking_level {
            crate::config::ThinkingLevel::Off => ThinkingLevel::Off,
            crate::config::ThinkingLevel::Minimal => ThinkingLevel::Minimal,
            crate::config::ThinkingLevel::Low => ThinkingLevel::Low,
            crate::config::ThinkingLevel::Medium => ThinkingLevel::Medium,
            crate::config::ThinkingLevel::High => ThinkingLevel::High,
        };
        let mut agent = Agent::new(OpenAiCompatProvider)
            .with_model_config(model_config.clone())
            .with_system_prompt(SYSTEM_PROMPT)
            .with_model(&llm_config.model)
            .with_api_key(&llm_config.api_key)
            .with_thinking(thinking_level)
            .with_tools(build_tools(allowed_dir.clone()))
            .with_execution_limits(ExecutionLimits {
                max_turns: llm_config.max_turns,
                ..ExecutionLimits::default()
            });
        agent.temperature = llm_config.temperature;

        if !runtime.skills.is_empty()
            && let Ok(skill_set) = SkillSet::load(runtime.skills.as_slice())
        {
            agent = agent.with_skills(skill_set);
        }

        let mut current_agent = Some(agent);
        let mut mcp_error: Option<anyhow::Error> = None;

        // yoagent's with_mcp_server_* consumes self and returns Result<Self, _>.
        // Wrap in Option so the borrow checker can see agent is always valid after the loop.
        for cmd in &mcp_config.stdio_transports {
            let Some(agent) = current_agent.take() else {
                break;
            };
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            match parts.split_first() {
                Some((command, mcp_args)) => {
                    match agent.with_mcp_server_stdio(command, mcp_args, None).await {
                        Ok(new_agent) => current_agent = Some(new_agent),
                        Err(e) => {
                            mcp_error = Some(anyhow!(e));
                            break;
                        }
                    }
                }
                None => current_agent = Some(agent),
            }
        }

        debug_assert!(
            current_agent.is_some() && mcp_error.is_none()
                || current_agent.is_none() && mcp_error.is_some()
        );

        for url in &mcp_config.sse_transports {
            let Some(agent) = current_agent.take() else {
                break;
            };
            match agent.with_mcp_server_http(url.as_str()).await {
                Ok(new_a) => {
                    current_agent = Some(new_a);
                }
                Err(e) => {
                    mcp_error = Some(anyhow!(e));
                    break;
                }
            }
        }

        let result: anyhow::Result<Agent> = match (current_agent, mcp_error) {
            (Some(agent), None) => Ok(agent),
            (None, Some(e)) => Err(e),
            _ => unreachable!(),
        };

        let coding_agent = CodingAgent(Arc::new(Mutex::new(result)));
        () = ctx
            .run_on_main_thread(move |ctx| {
                () = ctx.world.insert_resource(coding_agent);
            })
            .await;

        tokio::select! {
            _ = app_cancel.cancelled() => (),
            _ = agents_cancel.cancelled() => (),
            else => unreachable!(),
        }
    });
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq, States)]
enum CodingAgentState {
    #[default]
    Initializing,
    Idle,
    Processing,
}

#[derive(Default, Resource)]
pub struct CodingAgentTask {
    in_text: bool,
    in_thinking: bool,
    buffer: String,
}

#[derive(Default, Deref, DerefMut, Resource)]
pub struct CodingAgentTotalTokenUsage(pub Usage);

#[derive(Debug, Deref, DerefMut, Message)]
pub struct CodingAgentEvent(AgentEvent);

fn spawn_agent_task(
    channel: Res<'_, CodingAgentPromptChannel>,
    runtime: ResMut<'_, TokioTasksRuntime>,
    coding_agent: Res<'_, CodingAgent>,
    app_config: Res<'_, AppConfig>,
    mut commands: Commands<'_, '_>,
    mut next_state: ResMut<'_, NextState<CodingAgentState>>,
) {
    if let Ok(prompt) = channel.receiver.try_recv() {
        // Capture needed config values before moving into async task to avoid lifetime issues.
        let error_handling = app_config.runtime.strict_errors;
        let coding_agent = Arc::clone(&**coding_agent.into_inner());
        _ = runtime.spawn_background_task(move |mut ctx| async move {
            // // Try to prompt the LLM with retry logic.
            let rx_result = retry_async(|| async {
                // The prompt itself does not return a Result, so we wrap it.
                match *coding_agent.clone().lock().await {
                    Ok(ref mut agent) => Ok(agent.prompt(prompt.clone()).await),
                    Err(_) => Err(()),
                }
            })
            .await;
            match rx_result {
                Ok(mut rx) => {
                    // Capture token usage directly from AgentEnd while forwarding
                    // events to the message queue.  We cannot rely on
                    // handle_coding_agent_events processing AgentEnd because
                    // CodingAgentTask is removed (and the system's run-condition
                    // fails) before the message is consumed.
                    let mut final_usage: Option<Usage> = None;

                    while let Some(event) = rx.recv().await {
                        if let AgentEvent::AgentEnd { ref messages } = event {
                            for msg in messages.iter().rev() {
                                if let AgentMessage::Llm(yoagent::types::Message::Assistant {
                                    usage,
                                    ..
                                }) = msg
                                {
                                    final_usage = Some(usage.clone());
                                    break;
                                }
                            }
                        }
                        () = ctx
                            .run_on_main_thread(|ctx| {
                                let _ = ctx
                                    .world
                                    .write_message::<CodingAgentEvent>(CodingAgentEvent(event));
                            })
                            .await;
                    }
                    // Flush internal agent state so messages accumulate for the next turn.
                    if let Ok(ref mut agent) = *coding_agent.lock().await {
                        agent.finish().await;
                    }
                    // When done, accumulate token usage and reset state to Idle.
                    () = ctx
                        .run_on_main_thread(move |ctx| {
                            let world: &mut World = ctx.world;
                            if let Some(usage) = final_usage {
                                if let Some(mut total) =
                                    world.get_resource_mut::<CodingAgentTotalTokenUsage>()
                                {
                                    let CodingAgentTotalTokenUsage(Usage {
                                        input: dst_in,
                                        output: dst_out,
                                        cache_read: dst_cr,
                                        cache_write: dst_cw,
                                        total_tokens: dst_tt,
                                    }) = &mut *total;
                                    *dst_in += usage.input;
                                    *dst_out += usage.output;
                                    *dst_cr += usage.cache_read;
                                    *dst_cw += usage.cache_write;
                                    *dst_tt += usage.total_tokens;
                                }
                            }
                            _ = world.remove_resource::<CodingAgentTask>();
                            () = world
                                .get_resource_mut::<NextState<CodingAgentState>>()
                                .unwrap()
                                .set(CodingAgentState::Idle);
                        })
                        .await;
                }
                Err(_e) => {
                    // Use unified error handling respecting the runtime flag.
                    // Propagate the error through handle_error; it will log if enabled or return Err.
                    let simple_err = IoError::other("LLM request failed");
                    let _ = handle_error(simple_err, error_handling);
                    () = ctx
                        .run_on_main_thread(|ctx: MainThreadContext| {
                            let world = ctx.world;
                            // Ensure resources are cleaned up and state set to Idle.
                            _ = world.remove_resource::<CodingAgentTask>();
                            () = world
                                .get_resource_mut::<NextState<CodingAgentState>>()
                                .unwrap()
                                .set(CodingAgentState::Idle);
                        })
                        .await;
                }
            }

            // The event processing and cleanup is handled inside the match arms above.
        });

        () = commands.init_resource::<CodingAgentTask>();
        () = next_state.set(CodingAgentState::Processing);
    }
}

/// Helper to handle errors based on the runtime `error_handling` flag.
/// If error handling is enabled, logs the error and returns `Ok(())`.
/// Otherwise, returns an `Err` with the error message.
#[allow(dead_code)]
pub fn handle_error<E>(err: E, error_handling: bool) -> anyhow::Result<()>
where
    E: fmt::Display + std::error::Error + Send + Sync + 'static,
{
    if error_handling {
        eprintln!("Error: {err}");
        Ok(())
    } else {
        Err(anyhow::anyhow!(err))
    }
}

pub(crate) fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Render markdown text to Ratatui `Line`s via termimad → ansi-to-tui.
///
/// Falls back to plain line splitting on any conversion error so the caller
/// always gets a usable `Vec<Line<'static>>` regardless of input.
fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let skin = termimad::MadSkin::default();
    let mut buf: Vec<u8> = Vec::new();
    if skin.write_text_on(&mut buf, text).is_ok() {
        if let Ok(parsed) = buf.into_text() {
            return parsed.lines;
        }
    }
    // Fallback: raw line splitting
    text.lines().map(|l| Line::raw(l.to_string())).collect()
}

fn handle_coding_agent_events(
    mut messages: MessageReader<CodingAgentEvent>,
    mut coding_agent_task: ResMut<CodingAgentTask>,
    mut tui: Option<NonSendMut<TuiMain>>,
    // Option<> so this system works in non-TUI mode (--prompt flag) where
    // tui_plugin is not loaded and RenderNeeded does not exist.
    mut dirty: Option<ResMut<RenderNeeded>>,
    permission: Res<PermissionConfig>,
) {
    for CodingAgentEvent(event) in messages.read() {
        let CodingAgentTask {
            in_text,
            in_thinking,
            buffer: buf,
            ..
        } = coding_agent_task.as_mut();

        match event {
            AgentEvent::ToolExecutionStart {
                tool_name, args, ..
            } => {
                // Permission check for tools: apply whitelist where appropriate.
                let permission_check = |tool: &str, arg: &str| {
                    if tool == "bash" {
                        permission.validate_command(arg)
                    } else {
                        permission.validate_path(arg).map_err(anyhow::Error::from)
                    }
                };
                let maybe_path = match tool_name.as_str() {
                    "read_file" | "write_file" | "edit_file" | "list_files" => {
                        args.get("path").and_then(|v| v.as_str())
                    }
                    "search" => args.get("path").and_then(|v| v.as_str()),
                    "bash" => args.get("command").and_then(|v| v.as_str()),
                    _ => None,
                };
                if let Some(p) = maybe_path
                    && let Err(err) = permission_check(tool_name.as_str(), p)
                {
                    let msg = err.to_string();
                    if let Some(tui) = tui.as_mut() {
                        () = tui.push_line(Line::from(msg).red());
                    } else {
                        eprintln!("{err}");
                    }
                }

                // Seal any open thinking block — model may call tools without
                // emitting a Text delta first, so end_thinking won't trigger there.
                if *in_thinking {
                    if let Some(tui) = tui.as_mut() {
                        () = tui.end_thinking(0);
                    }
                    *in_thinking = false;
                }

                // Seal any open streaming text before showing the tool line.
                if *in_text {
                    if let Some(tui) = tui.as_mut() {
                        () = tui.finalize_streaming_text();
                    }
                    *in_text = false;
                }

                // Build a compact one-line summary for the ToolCall block.
                let summary: String = match tool_name.as_str() {
                    "bash" => {
                        let cmd = args
                            .get("command")
                            .and_then(|v| v.as_str())
                            .unwrap_or("...");
                        format!("$ {}", truncate(cmd, 60))
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
                    _ => tool_name.clone(),
                };

                if let Some(tui) = tui.as_mut() {
                    () = tui.begin_tool_call(summary);
                    () = tui.scroll_to_bottom();
                }
                if let Some(ref mut d) = dirty {
                    ***d = true;
                }
            }
            AgentEvent::ToolExecutionEnd {
                result, is_error, ..
            } => {
                // Truncate large result dumps to a short error snippet (errors only).
                let error_snippet = if *is_error {
                    truncate(&format!("{result:?}"), 80).to_string()
                } else {
                    String::new()
                };
                if let Some(tui) = tui.as_mut() {
                    () = tui.finish_tool_call(*is_error, error_snippet);
                    () = tui.scroll_to_bottom();
                }
                if let Some(ref mut d) = dirty {
                    ***d = true;
                }
            }
            AgentEvent::MessageUpdate {
                delta: StreamDelta::Thinking { delta },
                ..
            } => {
                if !*in_thinking {
                    if let Some(tui) = tui.as_mut() {
                        () = tui.begin_thinking();
                    }
                    *in_thinking = true;
                }
                if let Some(tui) = tui.as_mut() {
                    () = tui.append_thinking(delta);
                    () = tui.scroll_to_bottom();
                }
                if let Some(ref mut d) = dirty {
                    ***d = true;
                }
            }
            AgentEvent::MessageUpdate {
                delta: StreamDelta::Text { delta },
                ..
            } => {
                if *in_thinking {
                    // Seal the ThinkingBlock when text begins.
                    // Token count is not available here; AgentEnd has the full tally.
                    if let Some(tui) = tui.as_mut() {
                        () = tui.end_thinking(0);
                    }
                    *in_thinking = false;
                }

                if !*in_text {
                    () = buf.clear();
                    if let Some(tui) = tui.as_mut() {
                        () = tui.begin_streaming_text();
                    }
                    *in_text = true;
                }

                if tui.is_none() {
                    print!("{delta}");
                    () = stdout().flush().unwrap();
                } else if let Some(tui) = tui.as_mut() {
                    () = buf.push_str(delta);

                    // Render the accumulated buffer as plain lines.
                    // termimad + ansi-to-tui is used for final markdown presentation;
                    // during streaming we use raw splits so content is always visible.
                    let rendered: Vec<Line<'static>> =
                        buf.lines().map(|l| Line::raw(l.to_string())).collect();
                    () = tui.update_streaming_text(rendered);
                    () = tui.scroll_to_bottom();
                }
                if let Some(ref mut d) = dirty {
                    ***d = true;
                }
            }
            AgentEvent::AgentEnd { .. } => {
                // Token accumulation is handled directly in the background task's
                // cleanup closure (spawn_agent_task) to avoid the race where
                // CodingAgentTask is removed before this handler can run.
                // We still seal any TUI blocks that might be left open.
                if *in_thinking {
                    if let Some(tui) = tui.as_mut() {
                        () = tui.end_thinking(0);
                    }
                    *in_thinking = false;
                }
                if *in_text {
                    if let Some(tui) = tui.as_mut() {
                        // Render the full buffer as markdown before sealing the block.
                        // During streaming we use raw lines for speed; at the end we
                        // upgrade to fully rendered markdown via termimad → ansi-to-tui.
                        let md_lines = render_markdown(&buf);
                        () = tui.update_streaming_text(md_lines);
                        () = tui.finalize_streaming_text();
                    }
                    *in_text = false;
                }
            }
            _ => (),
        }
    }
}

fn check_agent_ready(
    coding_agent: Option<Res<CodingAgent>>,
    mut next_state: ResMut<NextState<CodingAgentState>>,
    mcp_config: Res<McpConfig>,
    mut tui: Option<NonSendMut<TuiMain>>,
    app_config: Res<AppConfig>,
) {
    // Wait until the async setup task has inserted the CodingAgent resource.
    if coding_agent.is_none() {
        return;
    }

    () = next_state.set(CodingAgentState::Idle);

    let McpConfig {
        sse_transports,
        stdio_transports,
    } = mcp_config.as_ref();
    let has_mcp = !sse_transports.is_empty() || !stdio_transports.is_empty();

    if let Some(tui) = tui.as_mut() {
        () = tui.push_line(Line::from("✅ Ready").green());
        if has_mcp {
            () = tui.push_line(Line::from("✅ MCP connected:").green());
            for url in sse_transports {
                () = tui.push_line(Line::from(format!("  http: {url}")).green());
            }
            for cmd in stdio_transports {
                let label = cmd.split_whitespace().next().unwrap_or(cmd.as_str());
                () = tui.push_line(Line::from(format!("  stdio: {label}")).green());
            }
        }
        () = tui.scroll_to_bottom();
    } else if app_config.runtime.verbose {
        eprintln!("greatsage: ready");
        if has_mcp {
            eprintln!("greatsage: MCP connected:");
            for url in sse_transports {
                eprintln!("  http: {url}");
            }
            for cmd in stdio_transports {
                let label = cmd.split_whitespace().next().unwrap_or(cmd.as_str());
                eprintln!("  stdio: {label}");
            }
        }
    }
}

fn shutdown_coding_agent(
    mut messages: MessageReader<AppExit>,
    mut cancel: Option<Res<AgentsCancelToken>>,
) {
    for _message in messages.read() {
        if let Some(cancel) = cancel.take()
            && !cancel.is_cancelled()
        {
            () = cancel.cancel();
        }
    }
}

pub fn coding_agent_plugin(app: &mut App) {
    let _: &mut App = app
        .init_resource::<CodingAgentPromptChannel>()
        .init_resource::<CodingAgentTotalTokenUsage>()
        .add_systems(Startup, setup)
        .init_state::<CodingAgentState>()
        .add_message::<CodingAgentEvent>()
        .add_systems(
            Update,
            (
                check_agent_ready.run_if(in_state(CodingAgentState::Initializing)),
                spawn_agent_task.run_if(
                    in_state(CodingAgentState::Idle)
                        .and(not(resource_exists::<CodingAgentTask>))
                        .and(resource_exists::<CodingAgent>),
                ),
                handle_coding_agent_events.run_if(
                    in_state(CodingAgentState::Processing).and(resource_exists::<CodingAgentTask>),
                ),
            ),
        )
        .add_systems(PostUpdate, shutdown_coding_agent);
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use {super::*, pretty_assertions::assert_eq};

    #[test]
    fn truncates_short_string() {
        let s: &str = "Hello";
        assert_eq!(truncate(s, 10), "Hello");
    }

    #[test]
    fn truncates_exact_length() {
        let s: &str = "Hello";
        assert_eq!(truncate(s, 5), "Hello");
    }

    #[test]
    fn truncates_unicode_without_splitting() {
        let s: &str = "🦀Rust";
        // The crab emoji is a single Unicode scalar value.
        assert_eq!(truncate(s, 1), "🦀");
    }
}
