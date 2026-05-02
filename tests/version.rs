#[cfg(test)]
mod tests {
    use {assert_cmd::Command, predicates::str::contains};

    #[test]
    fn version_flag_prints_version() {
        let mut cmd = Command::cargo_bin("greatsage").expect("binary exists");
        cmd.arg("--version");
        cmd.assert()
            .success()
            .stdout(contains(env!("CARGO_PKG_VERSION")));
    }
}
