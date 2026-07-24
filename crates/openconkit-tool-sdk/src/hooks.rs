//! Test hooks a tool may expose to the repository's integration-test tiers.
//!
//! All methods have defaults so tools opt in incrementally; a tool with no
//! hooks simply returns `None` from [`crate::Tool::test_hooks`].

/// Hooks used by integration tests and the fixture pipeline.
///
/// Every method has a default implementation returning `None`, so a tool
/// implements only what it ships.
pub trait ToolTestHooks: Send + Sync {
    /// Deterministic fixture input used by integration tests, if the tool
    /// ships one. The value is the tool-typed input, serialized.
    fn fixture_input(&self) -> Option<serde_json::Value> {
        None
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    struct NoHooks;
    impl ToolTestHooks for NoHooks {}

    struct WithFixture;
    impl ToolTestHooks for WithFixture {
        fn fixture_input(&self) -> Option<serde_json::Value> {
            Some(json!({ "threshold": 1 }))
        }
    }

    #[test]
    fn default_hooks_return_none() {
        assert!(NoHooks.fixture_input().is_none());
    }

    #[test]
    fn opted_in_hook_returns_fixture() {
        assert_eq!(
            WithFixture.fixture_input().expect("fixture"),
            json!({ "threshold": 1 })
        );
    }
}
