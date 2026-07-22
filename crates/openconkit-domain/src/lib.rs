//! OpenConKit domain layer.
//!
//! Pure entities, value objects and typed errors. This crate has no
//! infrastructure dependencies: no filesystem, no database, no UI.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod error;
pub mod project;

pub use error::DomainError;
pub use project::{Project, ProjectId};
