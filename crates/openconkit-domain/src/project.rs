//! Project aggregate: the unit of work a construction professional reviews.

use jiff::Timestamp;
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use crate::DomainError;

/// A validated project identifier (kebab-case slug, e.g. `tower-a-boq`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, TS)]
pub struct ProjectId(String);

impl ProjectId {
    /// Create a project id, enforcing the slug format.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let valid = !raw.is_empty()
            && raw.len() <= 64
            && !raw.starts_with('-')
            && !raw.ends_with('-')
            && !raw.contains("--")
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

impl<'de> Deserialize<'de> for ProjectId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Optional descriptive metadata for a project. Every field is optional;
/// absent fields mean "not recorded", never "empty".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ProjectMetadata {
    /// Free-text description of the project.
    #[serde(default)]
    pub description: Option<String>,
    /// Client the work is performed for.
    #[serde(default)]
    pub client: Option<String>,
    /// Site or city where the project is located.
    #[serde(default)]
    pub location: Option<String>,
}

/// A construction project under review.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Project {
    id: ProjectId,
    name: String,
    created_at: Timestamp,
    updated_at: Timestamp,
    archived_at: Option<Timestamp>,
    metadata: ProjectMetadata,
}

#[derive(Deserialize)]
struct ProjectUnchecked {
    id: ProjectId,
    name: String,
    created_at: Timestamp,
    updated_at: Timestamp,
    archived_at: Option<Timestamp>,
    metadata: ProjectMetadata,
}

impl TryFrom<ProjectUnchecked> for Project {
    type Error = DomainError;

    fn try_from(value: ProjectUnchecked) -> Result<Self, Self::Error> {
        Self::reconstitute(
            value.id,
            value.name,
            value.created_at,
            value.updated_at,
            value.archived_at,
            value.metadata,
        )
    }
}

impl<'de> Deserialize<'de> for Project {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let unchecked = ProjectUnchecked::deserialize(deserializer)?;
        Self::try_from(unchecked).map_err(serde::de::Error::custom)
    }
}

impl Project {
    /// Create a project, enforcing domain invariants.
    ///
    /// `updated_at` is initialized to `created_at`; metadata starts empty.
    /// Use [`Project::with_metadata`] to attach metadata fluently.
    pub fn new(
        id: ProjectId,
        name: impl Into<String>,
        created_at: Timestamp,
    ) -> Result<Self, DomainError> {
        Self::reconstitute(
            id,
            name,
            created_at,
            created_at,
            None,
            ProjectMetadata::default(),
        )
    }

    /// Rebuild a project from persisted fields (storage reconstitution).
    ///
    /// Enforces the same name invariant as [`Project::new`]; timestamps and
    /// metadata are accepted as stored.
    pub fn reconstitute(
        id: ProjectId,
        name: impl Into<String>,
        created_at: Timestamp,
        updated_at: Timestamp,
        archived_at: Option<Timestamp>,
        metadata: ProjectMetadata,
    ) -> Result<Self, DomainError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(DomainError::EmptyProjectName);
        }
        Ok(Self {
            id,
            name: name.trim().to_string(),
            created_at,
            updated_at,
            archived_at,
            metadata,
        })
    }

    /// Attach metadata, builder style.
    pub fn with_metadata(mut self, metadata: ProjectMetadata) -> Self {
        self.metadata = metadata;
        self
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

    /// Last-modification timestamp (archive/restore bump it).
    pub fn updated_at(&self) -> Timestamp {
        self.updated_at
    }

    /// When the project was archived, if it is archived.
    pub fn archived_at(&self) -> Option<Timestamp> {
        self.archived_at
    }

    /// Project metadata.
    pub fn metadata(&self) -> &ProjectMetadata {
        &self.metadata
    }

    /// Whether the project is currently archived.
    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }

    /// Return an archived copy of this project.
    ///
    /// Immutable style: the receiver is unchanged and a new `Project` with
    /// `archived_at = Some(now)` and `updated_at = now` is returned.
    /// Archiving an already-archived project is a no-op copy.
    pub fn archive(&self, now: Timestamp) -> Self {
        let mut next = self.clone();
        next.archived_at = Some(now);
        next.updated_at = now;
        next
    }

    /// Return a restored (un-archived) copy of this project.
    ///
    /// Same immutable style as [`Project::archive`]: the receiver is
    /// unchanged; the copy has `archived_at = None` and `updated_at = now`.
    pub fn restore(&self, now: Timestamp) -> Self {
        let mut next = self.clone();
        next.archived_at = None;
        next.updated_at = now;
        next
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
        for bad in [
            "", "-tower", "tower-", "tower--a", "Tower-A", "tower_a", "tower a", "../tower",
            "tower/a",
        ] {
            assert!(ProjectId::new(bad).is_err(), "{bad:?} should be rejected");
        }
        assert!(ProjectId::new("a".repeat(65)).is_err());
    }

    #[test]
    fn project_id_deserialization_enforces_slug_invariant() {
        let parsed: Result<ProjectId, _> = serde_json::from_str("\"../tower\"");
        assert!(parsed.is_err());
    }

    #[test]
    fn project_trims_name_and_rejects_empty() {
        let id = ProjectId::new("p1").expect("valid slug");
        let project = Project::new(id, "  North Tower  ", Timestamp::now()).expect("valid project");
        assert_eq!(project.name(), "North Tower");

        let id = ProjectId::new("p2").expect("valid slug");
        assert!(Project::new(id, "   ", Timestamp::now()).is_err());
    }

    #[test]
    fn new_project_defaults_metadata_and_timestamps() {
        let now = Timestamp::now();
        let project =
            Project::new(ProjectId::new("p1").expect("slug"), "P1", now).expect("valid project");
        assert_eq!(project.created_at(), now);
        assert_eq!(project.updated_at(), now);
        assert_eq!(project.archived_at(), None);
        assert!(!project.is_archived());
        assert_eq!(project.metadata(), &ProjectMetadata::default());
    }

    #[test]
    fn with_metadata_attaches_metadata() {
        let metadata = ProjectMetadata {
            description: Some("BOQ review".into()),
            client: Some("ACME".into()),
            location: None,
        };
        let project = Project::new(ProjectId::new("p1").expect("slug"), "P1", Timestamp::now())
            .expect("valid project")
            .with_metadata(metadata.clone());
        assert_eq!(project.metadata(), &metadata);
    }

    #[test]
    fn archive_and_restore_return_updated_copies() {
        let t0 = Timestamp::now();
        let t1 = t0 + jiff::Span::new().minutes(5);
        let project =
            Project::new(ProjectId::new("p1").expect("slug"), "P1", t0).expect("valid project");

        let archived = project.archive(t1);
        assert!(!project.is_archived(), "original is unchanged");
        assert!(archived.is_archived());
        assert_eq!(archived.archived_at(), Some(t1));
        assert_eq!(archived.updated_at(), t1);

        let restored = archived.restore(t1);
        assert!(archived.is_archived(), "original is unchanged");
        assert!(!restored.is_archived());
        assert_eq!(restored.archived_at(), None);
        assert_eq!(restored.updated_at(), t1);
    }

    #[test]
    fn project_serde_round_trip() {
        let project = Project::new(ProjectId::new("p1").expect("slug"), "P1", Timestamp::now())
            .expect("valid project")
            .with_metadata(ProjectMetadata {
                description: None,
                client: Some("ACME".into()),
                location: Some("Riyadh".into()),
            });
        let json = serde_json::to_string(&project).expect("serialize");
        let back: Project = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, project);
    }

    #[test]
    fn project_deserialization_enforces_name_invariant() {
        let now = Timestamp::now().to_string();
        let json = format!(
            r#"{{"id":"tower-a","name":"   ","created_at":"{now}","updated_at":"{now}","archived_at":null,"metadata":{{}}}}"#
        );
        let parsed: Result<Project, _> = serde_json::from_str(&json);
        assert!(parsed.is_err());
    }

    #[test]
    fn metadata_deserializes_with_missing_fields() {
        let metadata: ProjectMetadata = serde_json::from_str("{}").expect("defaults apply");
        assert_eq!(metadata, ProjectMetadata::default());
    }

    #[test]
    fn project_timestamps_map_to_ts_string() {
        let cfg = ts_rs::Config::default();
        let decl = <Project as TS>::decl(&cfg);
        assert!(decl.contains("created_at: string"), "{decl}");
        assert!(decl.contains("updated_at: string"), "{decl}");
        assert!(decl.contains("archived_at: string | null"), "{decl}");
    }
}
