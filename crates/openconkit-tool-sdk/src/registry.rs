//! Compile-time tool registry.

use crate::descriptor::ToolDescriptor;
use crate::tool::Tool;

/// Registry of all tools compiled into the application.
///
/// Registration is explicit and static: adding a tool is a code change that
/// goes through review, never a runtime download.
#[derive(Default)]
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool. Called from shell composition code at startup.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    /// Descriptors of every registered tool, in registration order.
    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        self.tools.iter().map(|tool| tool.descriptor()).collect()
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::TOOL_CONTRACT_VERSION;

    struct DummyTool;

    impl Tool for DummyTool {
        fn descriptor(&self) -> ToolDescriptor {
            ToolDescriptor {
                id: "dummy".to_string(),
                contract_version: TOOL_CONTRACT_VERSION,
                name_key: "tools.dummy.name".to_string(),
                description_key: "tools.dummy.description".to_string(),
            }
        }
    }

    #[test]
    fn registry_lists_registered_tools_in_order() {
        let mut registry = ToolRegistry::new();
        assert!(registry.is_empty());
        registry.register(Box::new(DummyTool));
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.descriptors()[0].id, "dummy");
    }
}
