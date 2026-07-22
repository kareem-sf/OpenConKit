//! The tool contract trait.

use crate::descriptor::ToolDescriptor;

/// A tool hosted in the OpenConKit shell.
///
/// Future contract versions will add lifecycle hooks (activation, project
/// context, command surface). Version 1 is intentionally minimal: the shell
/// only needs to know what a tool is called.
pub trait Tool {
    /// Stable metadata describing this tool.
    fn descriptor(&self) -> ToolDescriptor;
}
