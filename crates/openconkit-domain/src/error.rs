//! Typed errors for the domain layer.

/// Errors produced by domain invariants.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    /// A project id was empty or contained characters outside the allowed set.
    #[error("invalid project id: {0:?}")]
    InvalidProjectId(String),

    /// A project name was empty or only whitespace.
    #[error("project name must not be empty")]
    EmptyProjectName,
}
