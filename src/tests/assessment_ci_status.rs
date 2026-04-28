#[cfg(test)]
mod tests {
    use {crate::evolve::assessment_phase, std::fs, tempfile::TempDir};

    #[test]
    fn ci_status_present() {
        let tmp = TempDir::new().expect("temp dir");
        let base = tmp.path();
        // create workflow file
        let wf_dir = base.join(".github/workflows");
        () = fs::create_dir_all(&wf_dir).expect("create wf dir");
        let wf_file = wf_dir.join("ci.yml");
        () = fs::write(&wf_file, "name: CI\n").expect("write wf file");
        let res = assessment_phase(base).expect("assessment_phase should succeed");
        assert!(
            res.contains("CI last: present"),
            "CI status should be present, got {}",
            res
        );
    }

    #[test]
    fn ci_status_none() {
        let tmp = TempDir::new().expect("temp dir");
        let base = tmp.path();
        // No workflow directory or files
        let res = assessment_phase(base).expect("assessment_phase should succeed");
        assert!(
            res.contains("CI last: none"),
            "CI status should be none, got {}",
            res
        );
    }
}
