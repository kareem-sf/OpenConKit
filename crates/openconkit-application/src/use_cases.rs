//! Use cases: application-level orchestration.

use std::path::Path;

use jiff::Timestamp;
use openconkit_domain::{
    AiAnalysis, AnalysisRun, AnalysisRunId, DomainError, ErrorCode, ExportRecord, Finding, Project,
    ProjectId, SourceRevision, SourceRevisionId,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ports::{
    AiAnalysisRepository, AnalysisRunRepository, ExportRepository, FindingRepository,
    ImportedSource, ProjectRepository, RepositoryError, RunHistoryEntry, RunHistoryRepository,
    SourceImportPolicy, SourceRevisionRepository, SourceStorage, SourceStorageError,
};

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

impl ErrorCode for RegisterProjectError {
    fn code(&self) -> &'static str {
        match self {
            RegisterProjectError::Domain(err) => err.code(),
            RegisterProjectError::Repository(err) => err.code(),
        }
    }
}

/// Registers a new project with an atomic duplicate check.
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
        let project = Project::new(id, name, Timestamp::now())?;
        self.repository.create(&project)?;
        Ok(project)
    }
}

/// Lists projects, optionally including archived ones.
pub struct ListProjects<'a> {
    repository: &'a dyn ProjectRepository,
}

impl<'a> ListProjects<'a> {
    /// Create the use case over a repository port.
    pub fn new(repository: &'a dyn ProjectRepository) -> Self {
        Self { repository }
    }

    /// List projects; pass `include_archived` to include archived projects.
    pub fn execute(&self, include_archived: bool) -> Result<Vec<Project>, RepositoryError> {
        self.repository.list(include_archived)
    }
}

/// Errors from the archive-project use case.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveProjectError {
    /// A domain invariant was violated.
    #[error(transparent)]
    Domain(#[from] DomainError),

    /// The repository rejected the operation.
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

impl ErrorCode for ArchiveProjectError {
    fn code(&self) -> &'static str {
        match self {
            ArchiveProjectError::Domain(err) => err.code(),
            ArchiveProjectError::Repository(err) => err.code(),
        }
    }
}

/// Archives a project.
pub struct ArchiveProject<'a> {
    repository: &'a dyn ProjectRepository,
}

impl<'a> ArchiveProject<'a> {
    /// Create the use case over a repository port.
    pub fn new(repository: &'a dyn ProjectRepository) -> Self {
        Self { repository }
    }

    /// Archive the project with the given id. Returns
    /// [`RepositoryError::NotFound`] when the project does not exist.
    pub fn execute(&self, id: &str) -> Result<(), ArchiveProjectError> {
        let id = ProjectId::new(id)?;
        if self.repository.find_by_id(&id)?.is_none() {
            return Err(RepositoryError::NotFound(id.to_string()).into());
        }
        self.repository.archive(&id, Timestamp::now())?;
        Ok(())
    }
}

/// Errors from the import-source use case.
#[derive(Debug, thiserror::Error)]
pub enum ImportSourceError {
    /// A domain invariant was violated.
    #[error(transparent)]
    Domain(#[from] DomainError),

    /// The repository rejected the operation.
    #[error(transparent)]
    Repository(#[from] RepositoryError),

    /// The file vault failed to import the source.
    #[error(transparent)]
    Storage(#[from] SourceStorageError),

    /// Persistence failed after the source copy was created, and cleanup of
    /// that uncommitted copy also failed.
    #[error("source import failed ({cause}); rollback cleanup failed: {cleanup}")]
    Rollback {
        /// Original domain/repository failure.
        cause: String,
        /// Cleanup failure from the source vault.
        cleanup: SourceStorageError,
    },
}

impl ErrorCode for ImportSourceError {
    fn code(&self) -> &'static str {
        match self {
            ImportSourceError::Domain(err) => err.code(),
            ImportSourceError::Repository(err) => err.code(),
            ImportSourceError::Storage(err) => err.code(),
            ImportSourceError::Rollback { .. } => "IMPORT_SOURCE_ROLLBACK",
        }
    }
}

/// Imports a source workbook into a project: copies it into the file vault
/// (immutably, hashed) and records a [`SourceRevision`].
pub struct ImportSource<'a> {
    projects: &'a dyn ProjectRepository,
    sources: &'a dyn SourceStorage,
    revisions: &'a dyn SourceRevisionRepository,
}

impl<'a> ImportSource<'a> {
    /// Create the use case over the file-vault and revision ports.
    pub fn new(
        projects: &'a dyn ProjectRepository,
        sources: &'a dyn SourceStorage,
        revisions: &'a dyn SourceRevisionRepository,
    ) -> Self {
        Self {
            projects,
            sources,
            revisions,
        }
    }

    /// Import `source_path` into the project `project_id_raw`.
    ///
    /// `original_path_metadata` is stored as metadata only — it is never
    /// used for filesystem access. The original file is never modified.
    pub fn execute(
        &self,
        project_id_raw: &str,
        tool_id: &str,
        source_path: &Path,
        policy: &SourceImportPolicy,
        original_path_metadata: Option<String>,
    ) -> Result<SourceRevision, ImportSourceError> {
        let project_id = ProjectId::new(project_id_raw)?;
        if self.projects.find_by_id(&project_id)?.is_none() {
            return Err(RepositoryError::NotFound(project_id.to_string()).into());
        }
        let original_filename = source_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .ok_or_else(|| SourceStorageError::InvalidFileName {
                name: source_path.display().to_string(),
            })?;
        let ImportedSource {
            sha256,
            size_bytes,
            stored_relative_path,
        } = self.sources.import(&project_id, source_path, policy)?;
        let imported = ImportedSource {
            sha256,
            size_bytes,
            stored_relative_path,
        };
        match self
            .revisions
            .find_by_project_and_hash(&project_id, &imported.sha256)
        {
            Ok(Some(existing)) => {
                self.sources.discard(&imported)?;
                return Ok(existing);
            }
            Ok(None) => {}
            Err(err) => {
                return Err(
                    self.cleanup_after_failure(&imported, ImportSourceError::Repository(err))
                );
            }
        }
        let revision = match SourceRevision::new(
            SourceRevisionId::new(),
            project_id,
            imported.sha256.clone(),
            original_filename,
            original_path_metadata,
            imported.stored_relative_path.clone(),
            imported.size_bytes,
            Timestamp::now(),
            tool_id.to_string(),
            None,
        ) {
            Ok(revision) => revision,
            Err(err) => {
                return Err(self.cleanup_after_failure(&imported, ImportSourceError::Domain(err)));
            }
        };
        if let Err(err) = self.revisions.save(&revision) {
            return Err(self.cleanup_after_failure(&imported, ImportSourceError::Repository(err)));
        }
        Ok(revision)
    }

    fn cleanup_after_failure(
        &self,
        imported: &ImportedSource,
        cause: ImportSourceError,
    ) -> ImportSourceError {
        match self.sources.discard(imported) {
            Ok(()) => cause,
            Err(cleanup) => ImportSourceError::Rollback {
                cause: cause.to_string(),
                cleanup,
            },
        }
    }
}

/// Project id of the built-in project used for quick one-off analyses.
pub const QUICK_ANALYSES_PROJECT_ID: &str = "quick-analyses";

/// Display name of the built-in quick-analyses project.
pub const QUICK_ANALYSES_PROJECT_NAME: &str = "Quick Analyses";

/// A persisted analysis run together with its authoritative findings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct RunDetails {
    pub run: AnalysisRun,
    pub findings: Vec<Finding>,
    /// Exact typed output returned by the tool. Older pre-migration runs can
    /// legitimately have no stored output and remain readable.
    #[ts(type = "unknown")]
    pub output: Option<serde_json::Value>,
    pub exports: Vec<ExportRecord>,
    pub ai_analyses: Vec<AiAnalysis>,
}

/// List imported source revisions for a project.
pub struct ListSourceRevisions<'a> {
    repository: &'a dyn SourceRevisionRepository,
}

impl<'a> ListSourceRevisions<'a> {
    pub fn new(repository: &'a dyn SourceRevisionRepository) -> Self {
        Self { repository }
    }

    pub fn execute(&self, project_id: &str) -> Result<Vec<SourceRevision>, RepositoryError> {
        let project_id = ProjectId::new(project_id)
            .map_err(|err| RepositoryError::Invariant(err.to_string()))?;
        self.repository.list_by_project(&project_id)
    }
}

/// List persisted analysis runs for a project.
pub struct ListAnalysisRuns<'a> {
    repository: &'a dyn AnalysisRunRepository,
}

impl<'a> ListAnalysisRuns<'a> {
    pub fn new(repository: &'a dyn AnalysisRunRepository) -> Self {
        Self { repository }
    }

    pub fn execute(&self, project_id: &str) -> Result<Vec<AnalysisRun>, RepositoryError> {
        let project_id = ProjectId::new(project_id)
            .map_err(|err| RepositoryError::Invariant(err.to_string()))?;
        self.repository.list_by_project(&project_id)
    }
}

/// List complete persisted history projections for a project.
pub struct ListRunHistory<'a> {
    repository: &'a dyn RunHistoryRepository,
}

impl<'a> ListRunHistory<'a> {
    pub fn new(repository: &'a dyn RunHistoryRepository) -> Self {
        Self { repository }
    }

    pub fn execute(&self, project_id: &str) -> Result<Vec<RunHistoryEntry>, RepositoryError> {
        let project_id = ProjectId::new(project_id)
            .map_err(|err| RepositoryError::Invariant(err.to_string()))?;
        self.repository.list_history_by_project(&project_id)
    }
}

/// Reopen one persisted run and all of its findings.
pub struct OpenAnalysisRun<'a> {
    runs: &'a dyn AnalysisRunRepository,
    findings: &'a dyn FindingRepository,
    exports: &'a dyn ExportRepository,
    ai_analyses: &'a dyn AiAnalysisRepository,
}

impl<'a> OpenAnalysisRun<'a> {
    pub fn new(
        runs: &'a dyn AnalysisRunRepository,
        findings: &'a dyn FindingRepository,
        exports: &'a dyn ExportRepository,
        ai_analyses: &'a dyn AiAnalysisRepository,
    ) -> Self {
        Self {
            runs,
            findings,
            exports,
            ai_analyses,
        }
    }

    pub fn execute(&self, run_id: &str) -> Result<RunDetails, RepositoryError> {
        let run_id = AnalysisRunId::parse(run_id)
            .map_err(|err| RepositoryError::Invariant(err.to_string()))?;
        let run = self
            .runs
            .find_by_id(&run_id)?
            .ok_or_else(|| RepositoryError::NotFound(run_id.to_string()))?;
        let findings = self.findings.list_by_run(&run_id)?;
        let output = self.runs.find_output(&run_id)?;
        let exports = self.exports.list_by_run(&run_id)?;
        let ai_analyses = self.ai_analyses.list_by_run(&run_id)?;
        Ok(RunDetails {
            run,
            findings,
            output,
            exports,
            ai_analyses,
        })
    }
}

/// Errors from the quick-import use case.
#[derive(Debug, thiserror::Error)]
pub enum QuickImportError {
    /// A domain invariant was violated.
    #[error(transparent)]
    Domain(#[from] DomainError),

    /// The repository rejected the operation.
    #[error(transparent)]
    Repository(#[from] RepositoryError),

    /// The file vault failed to import the source.
    #[error(transparent)]
    Storage(#[from] SourceStorageError),

    /// An import failed and its rollback cleanup also failed.
    #[error("source import rollback failed: {0}")]
    Rollback(String),
}

impl ErrorCode for QuickImportError {
    fn code(&self) -> &'static str {
        match self {
            QuickImportError::Domain(err) => err.code(),
            QuickImportError::Repository(err) => err.code(),
            QuickImportError::Storage(err) => err.code(),
            QuickImportError::Rollback(_) => "IMPORT_SOURCE_ROLLBACK",
        }
    }
}

impl From<RegisterProjectError> for QuickImportError {
    fn from(err: RegisterProjectError) -> Self {
        match err {
            RegisterProjectError::Domain(err) => QuickImportError::Domain(err),
            RegisterProjectError::Repository(err) => QuickImportError::Repository(err),
        }
    }
}

impl From<ImportSourceError> for QuickImportError {
    fn from(err: ImportSourceError) -> Self {
        match err {
            ImportSourceError::Domain(err) => QuickImportError::Domain(err),
            ImportSourceError::Repository(err) => QuickImportError::Repository(err),
            ImportSourceError::Storage(err) => QuickImportError::Storage(err),
            ImportSourceError::Rollback { cause, cleanup } => {
                QuickImportError::Rollback(format!("{cause}; {cleanup}"))
            }
        }
    }
}

/// Imports a workbook for a quick one-off analysis: ensures the built-in
/// quick-analyses project exists, then imports the source into it.
pub struct QuickImport<'a> {
    projects: &'a dyn ProjectRepository,
    sources: &'a dyn SourceStorage,
    revisions: &'a dyn SourceRevisionRepository,
}

impl<'a> QuickImport<'a> {
    /// Create the use case over the project, file-vault and revision ports.
    pub fn new(
        projects: &'a dyn ProjectRepository,
        sources: &'a dyn SourceStorage,
        revisions: &'a dyn SourceRevisionRepository,
    ) -> Self {
        Self {
            projects,
            sources,
            revisions,
        }
    }

    /// Ensure the quick-analyses project exists and import `source_path`.
    /// Returns the project (created or reused) and the new revision.
    pub fn execute(
        &self,
        tool_id: &str,
        source_path: &Path,
        policy: &SourceImportPolicy,
    ) -> Result<(Project, SourceRevision), QuickImportError> {
        let id = ProjectId::new(QUICK_ANALYSES_PROJECT_ID)?;
        let project = match self.projects.find_by_id(&id)? {
            Some(project) => project,
            None => {
                match RegisterProject::new(self.projects)
                    .execute(QUICK_ANALYSES_PROJECT_ID, QUICK_ANALYSES_PROJECT_NAME)
                {
                    Ok(project) => project,
                    Err(RegisterProjectError::Repository(RepositoryError::Duplicate(_))) => {
                        self.projects.find_by_id(&id)?.ok_or_else(|| {
                            RepositoryError::Invariant(
                                "quick-analyses project disappeared after duplicate insert"
                                    .to_string(),
                            )
                        })?
                    }
                    Err(err) => return Err(err.into()),
                }
            }
        };
        let revision = ImportSource::new(self.projects, self.sources, self.revisions).execute(
            QUICK_ANALYSES_PROJECT_ID,
            tool_id,
            source_path,
            policy,
            Some(source_path.display().to_string()),
        )?;
        Ok((project, revision))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use std::cell::Cell;
    use std::cell::RefCell;
    use std::collections::HashMap;

    use openconkit_domain::Sha256Hash;

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
        fn create(&self, project: &Project) -> Result<(), RepositoryError> {
            let mut inner = self.inner.borrow_mut();
            if inner.contains_key(project.id().as_str()) {
                return Err(RepositoryError::Duplicate(project.id().clone()));
            }
            inner.insert(project.id().as_str().to_string(), project.clone());
            Ok(())
        }

        fn save(&self, project: &Project) -> Result<(), RepositoryError> {
            self.inner
                .borrow_mut()
                .insert(project.id().as_str().to_string(), project.clone());
            Ok(())
        }

        fn find_by_id(&self, id: &ProjectId) -> Result<Option<Project>, RepositoryError> {
            Ok(self.inner.borrow().get(id.as_str()).cloned())
        }

        fn list(&self, include_archived: bool) -> Result<Vec<Project>, RepositoryError> {
            Ok(self
                .inner
                .borrow()
                .values()
                .filter(|project| include_archived || !project.is_archived())
                .cloned()
                .collect())
        }

        fn archive(
            &self,
            id: &ProjectId,
            archived_at: jiff::Timestamp,
        ) -> Result<(), RepositoryError> {
            let mut inner = self.inner.borrow_mut();
            let project = inner
                .get_mut(id.as_str())
                .ok_or_else(|| RepositoryError::NotFound(id.to_string()))?;
            *project = project.clone().archive(archived_at);
            Ok(())
        }
    }

    struct InMemoryRevisions {
        inner: RefCell<Vec<SourceRevision>>,
    }

    impl InMemoryRevisions {
        fn new() -> Self {
            Self {
                inner: RefCell::new(Vec::new()),
            }
        }
    }

    impl SourceRevisionRepository for InMemoryRevisions {
        fn save(&self, revision: &SourceRevision) -> Result<(), RepositoryError> {
            self.inner.borrow_mut().push(revision.clone());
            Ok(())
        }

        fn find_by_id(
            &self,
            id: &SourceRevisionId,
        ) -> Result<Option<SourceRevision>, RepositoryError> {
            Ok(self.inner.borrow().iter().find(|r| &r.id == id).cloned())
        }

        fn find_by_project_and_hash(
            &self,
            project_id: &ProjectId,
            sha256: &Sha256Hash,
        ) -> Result<Option<SourceRevision>, RepositoryError> {
            Ok(self
                .inner
                .borrow()
                .iter()
                .find(|revision| &revision.project_id == project_id && &revision.sha256 == sha256)
                .cloned())
        }

        fn list_by_project(
            &self,
            project_id: &ProjectId,
        ) -> Result<Vec<SourceRevision>, RepositoryError> {
            Ok(self
                .inner
                .borrow()
                .iter()
                .filter(|r| &r.project_id == project_id)
                .cloned()
                .collect())
        }
    }

    struct FakeSourceStorage {
        result: Result<ImportedSource, SourceStorageError>,
        imports: Cell<usize>,
        discards: Cell<usize>,
    }

    impl FakeSourceStorage {
        fn ok() -> Self {
            Self {
                result: Ok(ImportedSource {
                    sha256: Sha256Hash::from_bytes([0x42; 32]),
                    size_bytes: 12_345,
                    stored_relative_path: "projects/tower-a/sources/boq.xlsx".into(),
                }),
                imports: Cell::new(0),
                discards: Cell::new(0),
            }
        }

        fn failing(err: SourceStorageError) -> Self {
            Self {
                result: Err(err),
                imports: Cell::new(0),
                discards: Cell::new(0),
            }
        }
    }

    impl SourceStorage for FakeSourceStorage {
        fn import(
            &self,
            _project_id: &ProjectId,
            source: &Path,
            _policy: &SourceImportPolicy,
        ) -> Result<ImportedSource, SourceStorageError> {
            self.imports.set(self.imports.get() + 1);
            let mut imported = self.result.clone()?;
            let marker = source
                .to_string_lossy()
                .bytes()
                .fold(0u8, |acc, byte| acc.wrapping_add(byte));
            imported.sha256 = Sha256Hash::from_bytes([marker; 32]);
            Ok(imported)
        }

        fn discard(&self, _imported: &ImportedSource) -> Result<(), SourceStorageError> {
            self.discards.set(self.discards.get() + 1);
            Ok(())
        }
    }

    fn xlsx_policy() -> SourceImportPolicy {
        SourceImportPolicy {
            accepted_extensions: vec![".xls".into(), ".xlsx".into()],
            max_file_size_bytes: 64 * 1024 * 1024,
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

    #[test]
    fn list_projects_filters_archived() {
        let repo = InMemoryProjects::new();
        let register = RegisterProject::new(&repo);
        register.execute("tower-a", "Tower A").expect("ok");
        register.execute("tower-b", "Tower B").expect("ok");
        ArchiveProject::new(&repo)
            .execute("tower-b")
            .expect("archives");

        let list = ListProjects::new(&repo);
        let active = list.execute(false).expect("list");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id().as_str(), "tower-a");
        let all = list.execute(true).expect("list all");
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn archive_project_marks_archived() {
        let repo = InMemoryProjects::new();
        RegisterProject::new(&repo)
            .execute("tower-a", "Tower A")
            .expect("ok");
        ArchiveProject::new(&repo)
            .execute("tower-a")
            .expect("archives");
        let project = repo
            .find_by_id(&ProjectId::new("tower-a").expect("slug"))
            .expect("lookup")
            .expect("exists");
        assert!(project.is_archived());
        assert!(project.archived_at().is_some());
    }

    #[test]
    fn archive_project_missing_is_not_found() {
        let repo = InMemoryProjects::new();
        let err = ArchiveProject::new(&repo)
            .execute("ghost")
            .expect_err("missing project");
        assert!(matches!(
            err,
            ArchiveProjectError::Repository(RepositoryError::NotFound(_))
        ));
        assert_eq!(err.code(), "REPOSITORY_NOT_FOUND");
    }

    #[test]
    fn import_source_builds_and_saves_revision() {
        let projects = InMemoryProjects::new();
        RegisterProject::new(&projects)
            .execute("tower-a", "Tower A")
            .expect("project");
        let storage = FakeSourceStorage::ok();
        let revisions = InMemoryRevisions::new();
        let use_case = ImportSource::new(&projects, &storage, &revisions);
        let revision = use_case
            .execute(
                "tower-a",
                "boq-inspector",
                Path::new("C:\\Users\\qs\\Downloads\\boq.xlsx"),
                &xlsx_policy(),
                Some("C:\\Users\\qs\\Downloads\\boq.xlsx".into()),
            )
            .expect("imports");
        assert_eq!(revision.project_id.as_str(), "tower-a");
        assert_eq!(revision.original_filename, "boq.xlsx");
        assert_eq!(
            revision.original_path.as_deref(),
            Some("C:\\Users\\qs\\Downloads\\boq.xlsx")
        );
        assert_eq!(revision.stored_path, "projects/tower-a/sources/boq.xlsx");
        assert_eq!(revision.size_bytes, 12_345);
        assert_eq!(revision.tool_id, "boq-inspector");

        let saved = revisions
            .find_by_id(&revision.id)
            .expect("lookup")
            .expect("stored");
        assert_eq!(saved, revision);
    }

    #[test]
    fn import_source_propagates_storage_errors() {
        let projects = InMemoryProjects::new();
        RegisterProject::new(&projects)
            .execute("tower-a", "Tower A")
            .expect("project");
        let storage = FakeSourceStorage::failing(SourceStorageError::Io {
            message: "disk full".into(),
        });
        let revisions = InMemoryRevisions::new();
        let err = ImportSource::new(&projects, &storage, &revisions)
            .execute(
                "tower-a",
                "boq-inspector",
                Path::new("boq.xlsx"),
                &xlsx_policy(),
                None,
            )
            .expect_err("storage failure propagates");
        assert!(matches!(err, ImportSourceError::Storage(_)));
        assert_eq!(err.code(), "SOURCE_STORAGE_IO");
        assert!(revisions.inner.borrow().is_empty());
    }

    #[test]
    fn import_source_checks_project_before_copying() {
        let projects = InMemoryProjects::new();
        let storage = FakeSourceStorage::ok();
        let revisions = InMemoryRevisions::new();
        let err = ImportSource::new(&projects, &storage, &revisions)
            .execute(
                "missing",
                "boq-inspector",
                Path::new("boq.xlsx"),
                &xlsx_policy(),
                None,
            )
            .expect_err("missing project rejected");
        assert!(matches!(
            err,
            ImportSourceError::Repository(RepositoryError::NotFound(_))
        ));
        assert_eq!(storage.imports.get(), 0);
    }

    #[test]
    fn duplicate_content_reuses_revision_and_discards_extra_copy() {
        let projects = InMemoryProjects::new();
        RegisterProject::new(&projects)
            .execute("tower-a", "Tower A")
            .expect("project");
        let storage = FakeSourceStorage::ok();
        let revisions = InMemoryRevisions::new();
        let use_case = ImportSource::new(&projects, &storage, &revisions);
        let first = use_case
            .execute(
                "tower-a",
                "boq-inspector",
                Path::new("boq.xlsx"),
                &xlsx_policy(),
                None,
            )
            .expect("first import");
        let second = use_case
            .execute(
                "tower-a",
                "boq-inspector",
                Path::new("boq.xlsx"),
                &xlsx_policy(),
                None,
            )
            .expect("duplicate import");
        assert_eq!(first.id, second.id);
        assert_eq!(revisions.inner.borrow().len(), 1);
        assert_eq!(storage.discards.get(), 1);
    }

    #[test]
    fn quick_import_creates_project_once_and_reuses_it() {
        let projects = InMemoryProjects::new();
        let storage = FakeSourceStorage::ok();
        let revisions = InMemoryRevisions::new();
        let use_case = QuickImport::new(&projects, &storage, &revisions);

        let (project, revision1) = use_case
            .execute("boq-inspector", Path::new("first.xlsx"), &xlsx_policy())
            .expect("first import creates project");
        assert_eq!(project.id().as_str(), QUICK_ANALYSES_PROJECT_ID);
        assert_eq!(project.name(), QUICK_ANALYSES_PROJECT_NAME);
        assert_eq!(revision1.project_id.as_str(), QUICK_ANALYSES_PROJECT_ID);

        let (project2, revision2) = use_case
            .execute("boq-inspector", Path::new("second.xlsx"), &xlsx_policy())
            .expect("second import reuses project");
        assert_eq!(project2.id().as_str(), QUICK_ANALYSES_PROJECT_ID);
        assert_ne!(revision1.id, revision2.id);

        let all = projects.list(true).expect("list");
        assert_eq!(all.len(), 1, "quick project created exactly once");
        assert_eq!(revisions.inner.borrow().len(), 2);
    }
}
