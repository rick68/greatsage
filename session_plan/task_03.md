Title: Remove unused import warning in main.rs
Files: src/main.rs
Issue: none

The compiler warns about an unused import (`use bevy::app::PluginGroup`). Delete this line to clean up warnings. Ensure the code still builds and all tests pass after the change.
