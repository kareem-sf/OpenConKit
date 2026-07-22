//! Project aggregate: the unit of work a construction professional reviews.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::DomainError;

/// A validated project identifier (kebab-case slug, e.g. `tower-a-boq`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(String);

impl ProjectId {
    /// Create a project id, enforcing the slug format.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let valid = !raw.is_empty()
            && raw
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if valid {
            Ok(Self(raw))
        } else {
            Err(DomainError::InvalidProjectId(raw))
        }
    }

    /// Borrow the id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A construction project under review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    id: ProjectId,
    name: String,
    created_at: Timestamp,
}

impl Project {
    /// Create a project, enforcing domain invariants.
    pub fn new(
        id: ProjectId,
        name: impl Into<String>,
        created_at: Timestamp,
    ) -> Result<Self, DomainError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(DomainError::EmptyProjectName);
        }
        Ok(Self {
            id,
            name: name.trim().to_string(),
            created_at,
        })
    }

    /// Project identifier.
    pub fn id(&self) -> &ProjectId {
        &self.id
    }

    /// Display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Creation timestamp.
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn project_id_accepts_kebab_case_slug() {
        let id = ProjectId::new("tower-a-boq").expect("valid slug");
        assert_eq!(id.as_str(), "tower-a-boq");
    }

    #[test]
    fn project_id_rejects_invalid_characters() {
        assert!(ProjectId::new("Tower A").is_err());
        assert!(ProjectId::new("").is_err());
    }

    #[test]
    fn project_trims_name_and_rejects_empty() {
        let id = ProjectId::new("p1").expect("valid slug");
        let project = Project::new(id, "  North Tower  ", Timestamp::now()).expect("valid project");
        assert_eq!(project.name(), "North Tower");

        let id = ProjectId::new("p2").expect("valid slug");
        assert!(Project::new(id, "   ", Timestamp::now()).is_err());
    }
}
