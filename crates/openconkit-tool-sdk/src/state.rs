//! Tool-owned state migrations.
//!
//! Tools may persist their own state (under app home). When a tool's state
//! layout changes, it appends a [`ToolStateMigration`]; migrations are
//! append-only, matching the repository-wide migration discipline
//! (ADR 0004). The host applies them in ascending `version` order before the
//! tool runs against stored state.

/// One append-only migration step for a tool's own persisted state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolStateMigration {
    /// Monotonically increasing migration version, starting at 1.
    pub version: u32,
    /// Human-readable description of what the migration changes.
    pub description: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_constructible_and_comparable() {
        let migration = ToolStateMigration {
            version: 1,
            description: "create tool state table",
        };
        assert_eq!(migration.version, 1);
        assert_eq!(migration.description, "create tool state table");
        assert_eq!(migration, migration.clone());
    }
}
