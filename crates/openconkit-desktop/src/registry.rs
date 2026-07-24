//! Compile-time tool registry composition.
//!
//! The shell discovers navigation and tool cards from this registry only
//! (ADR 0003). `pnpm tool:new` inserts registration lines at the marker below.

use openconkit_tool_sdk::{RegistryError, ToolRegistry};

/// Build the registry of every tool compiled into this binary.
pub fn build_registry() -> Result<ToolRegistry, RegistryError> {
    let mut registry = ToolRegistry::new();
    // tool-new: register here
    registry.register(Box::new(
        openconkit_tool_boq_inspector::BoqInspectorTool::new(),
    ))?;
    Ok(registry)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use openconkit_tool_sdk::TOOL_CONTRACT_VERSION;

    #[test]
    fn registry_hosts_boq_inspector() {
        let registry = build_registry().expect("builds");
        assert_eq!(registry.len(), 1);
        let tool = registry.get("boq-inspector").expect("present");
        let manifest = tool.manifest();
        assert_eq!(manifest.id, "boq-inspector");
        assert_eq!(manifest.contract_version, TOOL_CONTRACT_VERSION);
        assert_eq!(manifest.route, "/tools/boq-inspector");

        let nav = registry.navigation();
        assert_eq!(nav.len(), 1);
        assert_eq!(nav[0].tool_id, "boq-inspector");
    }
}
