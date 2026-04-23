use greatsage::agents::PermissionConfig;
use tempfile::tempdir;

#[test]
fn command_with_allowed_flag_path_is_allowed() {
    // Allowed directory
    let allowed_dir = tempdir().expect("create allowed temp dir");
    let allowed_path = allowed_dir.path().to_path_buf();
    // Create a subdirectory to use with -C flag
    let sub_dir = allowed_path.join("sub");
    std::fs::create_dir_all(&sub_dir).expect("create sub dir");
    let config = PermissionConfig {
        allowed_dir: allowed_path.clone(),
    };
    let cmd = format!("git -C {} status", sub_dir.to_str().unwrap());
    assert!(config.validate_command(&cmd).is_ok());
}

#[test]
fn command_with_disallowed_flag_path_is_denied() {
    let allowed_dir = tempdir().expect("create allowed temp dir");
    let denied_dir = tempdir().expect("create denied temp dir");
    let config = PermissionConfig {
        allowed_dir: allowed_dir.path().to_path_buf(),
    };
    let cmd = format!("git -C {} status", denied_dir.path().to_str().unwrap());
    let res = config.validate_command(&cmd);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.contains("Token"));
    assert!(err.contains("disallowed"));
}

#[test]
fn command_mixed_allowed_and_disallowed_tokens() {
    let allowed_dir = tempdir().expect("create allowed temp dir");
    let allowed_file = allowed_dir.path().join("good.txt");
    std::fs::write(&allowed_file, b"ok").expect("write allowed file");
    let denied_dir = tempdir().expect("create denied temp dir");
    let denied_file = denied_dir.path().join("bad.txt");
    std::fs::write(&denied_file, b"no").expect("write denied file");
    let config = PermissionConfig {
        allowed_dir: allowed_dir.path().to_path_buf(),
    };
    let cmd = format!(
        "git -C {} cat {}",
        allowed_dir.path().to_str().unwrap(),
        denied_file.to_str().unwrap()
    );
    let res = config.validate_command(&cmd);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.contains(denied_file.to_str().unwrap()));
}
