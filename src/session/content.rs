use {
    super::components::{ContentProvenance, IndexedContent},
    yoagent::types::{AgentMessage, Content, Message},
};

pub(crate) fn indexed_content_from_user_message(message: &AgentMessage) -> Vec<IndexedContent> {
    let AgentMessage::Llm(Message::User { content, timestamp }) = message else {
        return Vec::new();
    };
    content
        .iter()
        .enumerate()
        .map(|(index, block)| IndexedContent {
            block_index: index as u32,
            content: block.clone(),
            provenance: ContentProvenance::UserMessage {
                timestamp_ms: *timestamp,
            },
        })
        .collect()
}

pub(crate) fn indexed_content_from_streamed_assistant(
    text: &str,
    timestamp_ms: u64,
) -> Vec<IndexedContent> {
    if text.is_empty() {
        return Vec::new();
    }
    vec![IndexedContent {
        block_index: 0,
        content: Content::Text {
            text: text.to_string(),
        },
        provenance: ContentProvenance::AssistantMessage { timestamp_ms },
    }]
}

pub(crate) fn indexed_content_from_assistant_message(
    message: &AgentMessage,
) -> Vec<IndexedContent> {
    let AgentMessage::Llm(Message::Assistant {
        content, timestamp, ..
    }) = message
    else {
        return Vec::new();
    };
    content
        .iter()
        .enumerate()
        .map(|(index, block)| IndexedContent {
            block_index: index as u32,
            content: block.clone(),
            provenance: ContentProvenance::AssistantMessage {
                timestamp_ms: *timestamp,
            },
        })
        .collect()
}

pub(crate) fn indexed_content_from_tool_start(
    tool_call_id: impl AsRef<str>,
    tool_name: impl AsRef<str>,
    args: &serde_json::Value,
) -> IndexedContent {
    IndexedContent {
        block_index: 0,
        content: Content::ToolCall {
            id: tool_call_id.as_ref().to_string(),
            name: tool_name.as_ref().to_string(),
            arguments: args.clone(),
            provider_metadata: None,
        },
        provenance: ContentProvenance::ToolExecutionStart {
            tool_call_id: tool_call_id.as_ref().to_string(),
            tool_name: tool_name.as_ref().to_string(),
        },
    }
}

pub(crate) fn indexed_content_from_turn_tool_result(message: &Message) -> Vec<IndexedContent> {
    let Message::ToolResult {
        tool_call_id,
        tool_name,
        content,
        is_error,
        timestamp,
    } = message
    else {
        return Vec::new();
    };
    content
        .iter()
        .enumerate()
        .map(|(index, block)| IndexedContent {
            block_index: index as u32,
            content: block.clone(),
            provenance: ContentProvenance::TurnToolResult {
                tool_call_id: tool_call_id.clone(),
                tool_name: tool_name.clone(),
                is_error: *is_error,
                timestamp_ms: *timestamp,
            },
        })
        .collect()
}

/// Reflect-friendly label for a yoagent `Content` variant.
pub(crate) fn content_kind_label(content: &Content) -> &'static str {
    match content {
        Content::Text { .. } => "text",
        Content::Image { .. } => "image",
        Content::Thinking { .. } => "thinking",
        Content::ToolCall { .. } => "toolCall",
    }
}

/// Primary text payload for BRP queries (when applicable).
pub(crate) fn content_primary_text(content: &Content) -> String {
    match content {
        Content::Text { text } => text.clone(),
        Content::Thinking { thinking, .. } => thinking.clone(),
        Content::ToolCall { arguments, .. } => arguments.to_string(),
        Content::Image { .. } => String::new(),
    }
}
