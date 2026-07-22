//! Use cases: application-level orchestration.

use jiff::Timestamp;
use openconkit_domain::{DomainError, Project, ProjectId};

use crate::ports::{ProjectRepository, RepositoryError};

/// Errors from the register-project use case.
#[derive(Debug, thiserror::Error)]
pub enum RegisterProjectError {
    /// A domain invariant was violated.
    #[error(transparent)]
    Domain(#[from] DomainError),

    /// The repository rejected the operation.
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// Registers a new project after checking for duplicates.
pub struct RegisterProject<'a> {
    repository: &'a dyn ProjectRepository,
}

impl<'a> RegisterProject<'a> {
    /// Create the use case over a repository port.
    pub fn new(repository: &'a dyn ProjectRepository) -> Self {
        Self { repository }
    }

    /// Register a project with the given slug id and display name.
    pub fn execute(&self, id: &str, name: &str) -> Result<Project, RegisterProjectError> {
        let id = ProjectId::new(id)?;
        if self.repository.find_by_id(&id)?.is_some() {
            return Err(RepositoryError::Duplicate(id).into());
        }
        let project = Project::new(id, name, Timestamp::now())?;
        self.repository.save(&project)?;
        Ok(project)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;

    struct InMemoryProjects {
        inner: RefCell<HashMap<String, Project>>,
    }

    impl InMemoryProjects {
        fn new() -> Self {
            Self {
                inner: RefCell::new(HashMap::new()),
            }
        }
    }

    impl ProjectRepository for InMemoryProjects {
        fn save(&self, project: &Project) -> Result<(), RepositoryError> {
            self.inner
                .borrow_mut()
                .insert(project.id().as_str().to_string(), project.clone());
            Ok(())
        }

        fn find_by_id(&self, id: &ProjectId) -> Result<Option<Project>, RepositoryError> {
            Ok(self.inner.borrow().get(id.as_str()).cloned())
        }
    }

    #[test]
    fn registers_a_new_project() {
        let repo = InMemoryProjects::new();
        let use_case = RegisterProject::new(&repo);
        let project = use_case.execute("tower-a", "Tower A").expect("registers");
        assert_eq!(project.id().as_str(), "tower-a");
        assert!(repo
            .find_by_id(project.id())
            .expect("lookup works")
            .is_some());
    }

    #[test]
    fn rejects_duplicate_ids() {
        let repo = InMemoryProjects::new();
        let use_case = RegisterProject::new(&repo);
        use_case.execute("tower-a", "Tower A").expect("first ok");
        let err = use_case
            .execute("tower-a", "Tower A Again")
            .expect_err("duplicate rejected");
        assert!(matches!(
            err,
            RegisterProjectError::Repository(RepositoryError::Duplicate(_))
        ));
    }
}
