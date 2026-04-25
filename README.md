# greatsage
Rimuru's Unique Skill, you know the one.

## Usage

Run a single prompt and exit (no REPL) using either the `--prompt` flag or a positional argument:

```bash
greatsage --prompt "Hello"
# or, more succinctly:
greatsage "Hello"
```


- `--evolve` — Run the self‑evolution pipeline (currently a placeholder). Example:

```bash
greatsage --evolve
```

*Note: full functionality is under development.*

## REPL usage

The REPL supports several slash commands and git shortcuts. Errors from commands are displayed in red with a ❌ icon.

- `/clear` – clears the REPL output.
- `/exit` or `/quit` – exits the application.
- `git stage` – stages all changes.
- `git commit -m "msg"` – commits staged changes.
- `git revert` – reverts the last commit.

## Security

The agent now enforces a permission check on filesystem access. `PermissionConfig` validates that any accessed path resides within the configured `allowed_dir` (defaulting to the current working directory). Unit tests verify that allowed paths are accepted and disallowed paths are rejected.
