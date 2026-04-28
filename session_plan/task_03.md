Title: Document --evolve flag in README
Files:
- README.md
Issue: none

## Description
Update the project's README to include a section describing the newly functional `--evolve` CLI flag.

### Steps
1. In `README.md`, add a subsection under "Usage" titled "Self‑Evolution (`--evolve`)".
2. Explain that `--evolve` triggers the internal evolution pipeline (dry‑run by default) and can be used to run the full self‑evolution process.
3. Provide example command lines:
   ```bash
   cargo run -- --evolve            # dry‑run (assessment + planning)
   cargo run -- --evolve --push    # run full pipeline and push results
   ```
4. Mention that the flag replaces the external `scripts/evolve.sh` and will eventually be the primary entry point.
5. Ensure markdown formatting matches existing style.

### Acceptance Criteria
- The README builds without rendering errors.
- The new section appears clearly under the Usage heading.
- No other files are modified.

---
This documentation task informs users about the newly functional evolve flag, improving usability and aligning with the roadmap.
