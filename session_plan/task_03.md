Title: Add conversation history persistence
Files: src/agents/coding.rs, src/agents/mod.rs
Issue: none

## Description
Currently the agent loses all context between prompts. Each session starts fresh with no message accumulation. Implement conversation history persistence.

## What to implement

1. **Create a conversation history data structure** (mod.rs or coding.rs):
   - Store accumulated messages (user prompts + agent responses)
   - Track message metadata (timestamp, token count per message)
   - Implement truncation/compaction when history gets too large

2. **Integrate history into agent state** (coding.rs):
   - Add `ConversationHistory` resource to the agent
   - Append each user prompt and agent response to history
   - Pass relevant history to LLM on each request (with context window management)

3. **Add history display capability**:
   - Option to show recent conversation in TUI
   - Consider adding a `history` REPL command to view past exchanges

## Why
Claude Code maintains conversation context automatically. Without this:
- Agent cannot remember what was discussed earlier in the session
- Multi-step tasks require re-explaining context
- User experience is fragmented and frustrating

This is a fundamental capability gap that must be closed.

## Implementation approach
- Start simple: store Vec<Message> with basic truncation
- Use existing truncate utility if available
- Pass last N messages to LLM (configurable, default ~50 messages or context window limit)

## Verification
- Agent remembers previous prompts in same session
- History doesn't cause OOM on long sessions (truncation works)
- `cargo build` and `cargo test` pass

## Docs to update
- README.md: Document conversation persistence feature
- YOYO.md: Update if agent behavior changes
