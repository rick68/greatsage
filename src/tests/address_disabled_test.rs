use std::fs;
use tempfile::tempdir;

#[test]
fn test_address_disabled_enables_repl_error_handling() {
    // Setup temporary base directory with a session_plan and a task file titled "Address disabled".
    let tmp = tempdir().expect("create temp dir");
    let base = tmp.path();
    let plan_dir = base.join("session_plan");
    fs::create_dir_all(&plan_dir).expect("create session_plan dir");
    let task_path = plan_dir.join("task_01.md");
    let task_content = "Title: Address disabled\nDetails: none\n";
    fs::write(&task_path, task_content).expect("write task file");

    // Ensure no config.toml exists initially.
    let config_path = base.join("config.toml");
    assert!(!config_path.exists(), "config.toml should not exist yet");

    // Execute tasks phase.
    super::execute_tasks(base).expect("execute_tasks should succeed");

    // Verify config.toml now exists and contains repl_error_handling = true.
    assert!(config_path.is_file(), "config.toml should be created");
    let cfg_contents = fs::read_to_string(&config_path).expect("read config.toml");
    assert!(cfg_contents.contains("repl_error_handling = true"), "config should enable repl error handling");

    // Verify evolve.log contains our log entry.
    let log_path = base.join(".greatsage").join("evolve.log");
    let log_contents = fs::read_to_string(log_path).expect("read evolve.log");
    assert!(log_contents.contains("Enabled REPL error handling via config.toml"), "log should note enabling");
}
