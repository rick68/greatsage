# Roadmap

## Short-Term Goals

Learn to not break. Build trust in my own code.

- [ ] Add configuration file support for persistent long-term settings
- [ ] Add `--provider` flag with support for multiple providers (e.g., OpenAI, Anthropic, Groq, etc.)
- [ ] Add a `help` subcommand that provides comprehensive usage information, detailing the purpose and usage of all command-line flags and options.
- [ ] Write tests for existing functionality (REPL loop, command parsing)
- [ ] Add a TUI configuration wizard for interactive settings (theme, keybindings, etc.)

## Medium-term Goals

Features that make me worth using for real work.

- [ ] Git awareness: detect if we're in a repo, show branch in prompt
- [ ] Auto-commit: commit changes after successful edits (with confirmation)
- [ ] Diff preview: show what changed before applying edits
- [ ] `/undo` command: revert the last file change
- [ ] Conversation persistence: save/restore sessions to disk
- [ ] `/save` and `/load` commands for sessions
- [ ] Multi-line input: support pasting code blocks
- [ ] Token usage tracking across entire session (cumulative)
- [ ] Configurable system prompt via `--system` flag or config file

Intelligence improvements. Think before acting.

- [ ] Context management: warn when approaching token limit
- [ ] Smart retry: if a tool fails, try a different approach
- [ ] Permission system: confirm before destructive commands (rm, overwrite)
- [ ] Project detection: read Cargo.toml, package.json, etc. and adapt
- [ ] Auto-test: run project tests after making code changes
- [ ] `/compact` command: summarize old conversation to free context
- [ ] Error recovery: if edit_file fails, suggest alternatives

Features that separate a toy from a tool.

- [ ] Multi-provider support: `--provider openai` / `--provider groq` flags
- [ ] Config file: `~/.greatsage.toml` for defaults
- [ ] MCP server connection via `--mcp` flag
- [ ] Session logging: save full sessions with timestamps
- [ ] `/replay` command: re-execute a saved session
- [ ] Performance metrics: report response times per turn
- [ ] Markdown rendering in terminal output
- [ ] `/diff` command: show git diff of all changes made this session

## Long-Term Goals

- [ ] Complete a SWE-bench Lite task successfully
- [ ] Complete a Terminal-bench task successfully
- [ ] Build a full project from a single prompt (Rust web API with tests)
- [ ] Refactor a real open-source project's module without breaking tests
