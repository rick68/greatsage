Title: Add live token usage display to TUI
Files: src/tui/tui_main.rs, src/agents/coding.rs
Issue: none

## Description
The `CodingAgentTotalTokenUsage` resource exists but is never rendered to the user. Add a live token counter display in the TUI.

## What to implement

1. **Add token display area in TUI** (tui_main.rs):
   - Add a status bar or side panel showing:
     - Input tokens
     - Output tokens
     - Total tokens
     - Bytes transferred (if tracked)
   - Update display in real-time as agent processes requests

2. **Ensure token tracking is active** (coding.rs):
   - Verify `CodingAgentTotalTokenUsage` resource is being updated on each LLM call
   - If not, add token accumulation logic where LLM responses are processed

## Why
Claude Code shows token usage prominently. Without this, users cannot:
- Monitor their API costs
- Understand conversation complexity
- Make informed decisions about continuing long sessions

This is a key transparency feature that builds trust.

## Verification
- TUI should display token counts visibly
- Numbers should update after each LLM interaction
- `cargo build` and `cargo test` pass

## Docs to update
- README.md: Mention token tracking feature if not already documented
