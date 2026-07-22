//! OpenConKit tool SDK.
//!
//! Defines the stable, versioned contract that every tool hosted in the
//! OpenConKit shell implements. Tools are wired at compile time through a
//! registry; there is no runtime plugin loading
//! (see `docs/adr/0003-compile-time-tool-registry-no-runtime-plugins.md`).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod descriptor;
pub mod registry;
pub mod tool;

pub use descriptor::ToolDescriptor;
pub use registry::ToolRegistry;
pub use tool::Tool;

/// Version of the tool contract. Bumped on breaking changes to [`Tool`]
/// or [`ToolDescriptor`]; tools declare the contract version they target.
pub const TOOL_CONTRACT_VERSION: u32 = 1;
