//! BOQ Inspector: automated Bill of Quantities quality review.
//!
//! The detection engine (ingestion, checks, benchmarks) lands in a later
//! phase; this crate already implements the tool contract so the shell can
//! list and open the tool.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use openconkit_tool_sdk::{Tool, ToolDescriptor, TOOL_CONTRACT_VERSION};

/// Stable identifier of the BOQ Inspector tool.
pub const TOOL_ID: &str = "boq-inspector";

/// The BOQ Inspector tool.
#[derive(Debug, Default)]
pub struct BoqInspectorTool;

impl BoqInspectorTool {
    /// Create the tool instance.
    pub fn new() -> Self {
        Self
    }
}

impl Tool for BoqInspectorTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor {
            id: TOOL_ID.to_string(),
            contract_version: TOOL_CONTRACT_VERSION,
            name_key: "tools.boqInspector.name".to_string(),
            description_key: "tools.boqInspector.description".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_targets_current_contract() {
        let tool = BoqInspectorTool::new();
        let descriptor = tool.descriptor();
        assert_eq!(descriptor.id, "boq-inspector");
        assert_eq!(descriptor.contract_version, TOOL_CONTRACT_VERSION);
        assert_eq!(descriptor.name_key, "tools.boqInspector.name");
    }
}
