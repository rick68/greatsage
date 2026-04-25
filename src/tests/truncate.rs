// Tests for the truncate utility function.

#[cfg(test)]
mod tests {
    use {crate::agents::coding::truncate, pretty_assertions::assert_eq};

    #[test]
    fn truncates_short_ascii() {
        let s = "Hello";
        assert_eq!(truncate(s, 10), "Hello");
    }

    #[test]
    fn truncates_exact_ascii() {
        let s = "Hello";
        assert_eq!(truncate(s, 5), "Hello");
    }

    #[test]
    fn truncates_long_ascii() {
        let s = "Hello, world!";
        // limit 5 should give "Hello"
        assert_eq!(truncate(s, 5), "Hello");
    }

    #[test]
    fn truncates_unicode_without_splitting() {
        let s = "🦀Rust"; // first char is a crab emoji
        // limit 1 should return only the emoji
        assert_eq!(truncate(s, 1), "🦀");
        // limit 3 should return "🦀Ru"
        assert_eq!(truncate(s, 3), "🦀Ru");
    }
}
