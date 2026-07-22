//! OpenConKit application layer.
//!
//! Use cases and orchestration. Depends only on the domain layer and
//! abstracts infrastructure behind ports (traits) implemented by adapters
//! such as `openconkit-storage`.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod ports;
pub mod use_cases;

pub use ports::{ProjectRepository, RepositoryError};
pub use use_cases::{RegisterProject, RegisterProjectError};
