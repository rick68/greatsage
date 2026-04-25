// Tests for protected path detection in evolve.rs

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn protects_known_paths() {
        let protected = [
            ".github/workflows",
            "IDENTITY.md",
            "scripts",
            "skills",
        ];
        for p in protected.iter() {
            let path = Path::new(p);
            assert!(is_protected_path(path), "{p} should be protected");
        }
    }

    #[test]
    fn does_not_protect_similar_names() {
        let non_protected = [
            "script",
            "scripts_backup",
            "skillful",
            "my/.github/workflows_extra",
        ];
        for p in non_protected.iter() {
            let path = Path::new(p);
            assert!(!is_protected_path(path), "{p} should NOT be protected");
        }
    }
}
