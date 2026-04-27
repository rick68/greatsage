Title: Update documentation for new REPL `--error-handling` flag and evolve protected‑file guard
Files: README.md
Issue: none

Description:
Reflect the newly added `--error-handling` flag and the enforced protected‑file guard in the project's documentation.

Implementation steps:
- In `README.md`, add a section under "Command‑line options" describing `--error-handling`:
  * What it does (strict validation of required files before REPL starts).
  * Example usage.
- Add a subsection describing the evolve pipeline's protected‑file enforcement, noting which paths are protected and the behavior when a violation occurs.
- Ensure the documentation builds cleanly (no markdown syntax errors).
- No code changes, only documentation.

Goal: Keep users informed of new safety features, maintaining alignment between code and docs.
