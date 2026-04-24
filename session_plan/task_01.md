Title: Fix Clippy warnings in coding.rs (single_match patterns)
Files: src/agents/coding.rs
Issue: none

## Description
Fix 2 cosmetic Clippy warnings that violate Rust best practices:

### Warning 1 (Lines 115-117)
Current code uses `if !args.skills.is_empty() && let Ok(...)` which is deprecated syntax.

**Change to:**
```rust
if !args.skills.is_empty() {
    if let Ok(...) = ... {
        // ...
    }
}
```

### Warning 2 (Lines 239-242)
Current code uses `match ... { Ok(x) => ..., _ => () }` which should use `if let`.

**Change to:**
```rust
if let Ok(x) = ... {
    // ...
}
```

## Why
Clean code builds build confidence and follow Rust idioms. These warnings indicate non-idiomatic patterns that should be corrected.

## Verification
After changes:
- `cargo clippy` should show 0 warnings
- `cargo build` should succeed
- `cargo test` should pass (all 16 tests)

## Docs to update
None (cosmetic code cleanup)
