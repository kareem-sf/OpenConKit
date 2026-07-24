//! OpenConKit application layer.
//!
//! Use cases and orchestration. Depends only on the domain layer and
//! abstracts infrastructure behind ports (traits) implemented by adapters
//! such as `openconkit-storage`.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod ai;
pub mod bootstrap;
pub mod config;
pub mod ipc;
pub mod ports;
pub mod updater;
pub mod use_cases;

pub use ai::{
    AiAccountSnapshot, AiLoginChallenge, AiLoginMode, AiPlanType, AiRateLimitSnapshot,
    AiRateLimitWindow, AiReviewScope, AiRuntimeStatus,
};
pub use bootstrap::{BootstrapStatus, HomeLayout};
pub use config::{
    AdvancedSettings, AnalysisTolerances, AppSettings, ConfigError, Language, PrivacySettings,
    SettingsPatch, Theme, UpdateChannel, UpdateChannelState, SETTINGS_SCHEMA_VERSION,
};
pub use ipc::IpcError;
pub use ports::{
    AiAnalysisRepository, AnalysisRunRepository, ExportRepository, FindingRepository,
    ImportedSource, ProjectRepository, RepositoryError, RunHistoryEntry, RunHistoryRepository,
    SourceImportPolicy, SourceRevisionRepository, SourceStorage, SourceStorageError,
};
pub use updater::{AvailableUpdate, UpdateCheckResult, UpdateProgressEvent, UpdateProgressPhase};
pub use use_cases::{
    ArchiveProject, ArchiveProjectError, ImportSource, ImportSourceError, ListAnalysisRuns,
    ListProjects, ListRunHistory, ListSourceRevisions, OpenAnalysisRun, QuickImport,
    QuickImportError, RegisterProject, RegisterProjectError, RunDetails, QUICK_ANALYSES_PROJECT_ID,
    QUICK_ANALYSES_PROJECT_NAME,
};
