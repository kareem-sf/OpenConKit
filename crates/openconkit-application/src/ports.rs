//! Ports: traits the infrastructure layer implements.

use openconkit_domain::{Project, ProjectId};

/// Errors reported by repository adapters.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    /// The underlying store failed.
    #[error("storage failure: {0}")]
    Storage(String),

    /// A project with the same id already exists.
    #[error("project already exists: {0}")]
    Duplicate(ProjectId),
}

/// Persistence port for projects.
pub trait ProjectRepository {
    /// Persist a new project.
    fn save(&self, project: &Project) -> Result<(), RepositoryError>;

    /// Look up a project by id.
    fn find_by_id(&self, id: &ProjectId) -> Result<Option<Project>, RepositoryError>;
}
