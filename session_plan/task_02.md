Title: Add TokenUsage resource and set it on AgentEnd
Files: src/agents/coding.rs
Issue: none

## Description
Create a new resource `TokenUsage` that stores the latest `Usage` from the LLM.
1. Define `pub struct TokenUsage(pub Usage);` with `#[derive(Default, Resource)]`.
2. In `handle_coding_agent_events`, when matching `AgentEvent::AgentEnd`, after extracting `last_usage`, also insert/update the `TokenUsage` resource in the world.
3. Ensure the resource is initialized (default) in the agents plugin.
4. No runtime behavior change beyond exposing usage for the UI.
