# greatsage
Rimuru's Unique Skill, you know the one.

## Usage

```bash
greatsage [OPTIONS] [prompt]
```

## Installation

```bash
cargo install --path .
```

Run the self‑evolution pipeline with:

## Evolution Pipeline – Safety

The evolve pipeline includes a guard against modifying critical repository files. The function `is_protected_path` checks paths against a whitelist of protected locations. The following directories and files are **never** modified by the evolution process:

- `.github/workflows/`
- `IDENTITY.md`
- `PERSONALITY.md`
- `scripts/`
- `skills/`

If any task attempts to write to a protected path, the pipeline aborts with an error message indicating the protected file was targeted.

```bash
greatsage evolve
```

The evolve pipeline generates three placeholder task files (`task_01.md`, `task_02.md`, `task_03.md`) in the `session_plan/` directory. Each contains a title like `Placeholder Task 1`, `Placeholder Task 2`, or `Placeholder Task 3`, with `Files: none` and `Issue: none` lines.

- `greatsage evolve --dry-run`: Perform a dry run of the evolve pipeline (assessment and planning phases only, without executing tasks).


Start the REPL (no arguments), or pass a prompt to run once and exit:

```bash
greatsage                        # interactive REPL
greatsage "Explain this code"    # single-shot, same as --prompt
greatsage --prompt "Hello"
```

### Options

| Flag | Description |
|------|-------------|
| `--config <PATH>` | Path to config file (default: `~/.config/greatsage/config.toml`) |
| `--model <name>` | Model to use (overrides config) |
| `--context-strategy <s>` | Context management: `compaction` or `checkpoint` |
| `--thinking <lvl>` | Extended thinking: `off` · `minimal` · `low` · `medium` · `high` |
| `--max-tokens <n>` | Maximum output tokens per response |
| `--max-turns <n>` | Maximum agent turns per prompt |
| `--temperature <f>` | Sampling temperature (0.0–1.0) |
| `--skills <dir>` | Directory of skill files (repeatable) |
| `--mcp <server>` | MCP server: HTTP URL or stdio command (repeatable) |
| `-v, --verbose` | Print status messages to stderr in non-interactive mode |
| `--error-handling` | Enable REPL error‑handling validation (experimental; default false). When enabled, REPL validates each command and displays errors in a standardized format, improving guard‑rail safety. |
| `--check` | Alias for `--error-handling`; enables REPL error handling validation. |
| `--strict-errors` | Exit REPL on internal errors (experimental; default false). In strict mode, any internal error aborts the REPL session, providing a guard‑rail against hidden failures. |

## REPL usage

All commands start with `/`. Errors are displayed in red with a ❌ icon. Run `/help` inside the REPL for a formatted reference.

```
Commands (in REPL):

  Session:
    /help              Show this help
    /clear             Clear output
    /quit, /exit       Exit greatsage

  Git:
    /git stage         Stage all changes
    /git commit -m …   Commit staged changes
    /git revert        Revert last commit
```

## Security

The agent now enforces a permission check on filesystem access. `PermissionConfig` validates that any accessed path resides within the configured `allowed_dir` (defaulting to the current working directory). Unit tests verify that allowed paths are accepted and disallowed paths are rejected.
