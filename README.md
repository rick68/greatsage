# greatsage
Rimuru's Unique Skill, you know the one.

## Usage

```bash
greatsage [OPTIONS] [prompt]
```

Run the self‑evolution pipeline with:
```bash
greatsage evolve
```

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
| `--error-handling` | Enable REPL error‑handling validation (experimental; default false) |

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
