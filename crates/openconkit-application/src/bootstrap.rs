//! App-home bootstrap DTOs: the canonical layout of `~/.openconkit` and the
//! status reported after a bootstrap run.
//!
//! These are plain data types — the infrastructure crate performs the actual
//! directory creation, validation and recovery work.

use openconkit_domain::ProjectId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Report produced by bootstrapping the app home on launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct BootstrapStatus {
    /// Absolute path of the app home directory.
    pub home_path: String,
    /// True when the home directory did not exist before this launch.
    pub created_fresh: bool,
    /// True when the canonical directory structure was validated.
    pub structure_validated: bool,
    /// True when a previous bootstrap did not finish (interrupt marker found)
    /// and recovery was performed.
    pub recovered_from_interrupt: bool,
    /// Per-field settings fallbacks and corruption recoveries.
    pub config_warnings: Vec<String>,
    /// Descriptions of database migrations applied this launch.
    pub database_migrations: Vec<String>,
    /// Relative paths of backups made this launch.
    pub backups_created: Vec<String>,
}

/// The canonical relative layout of the app home directory.
///
/// This is the SINGLE SOURCE OF TRUTH for the app-home structure: both the
/// infrastructure crate (which creates/validates it) and the desktop host
/// (which displays it) must use these constants instead of hard-coding
/// paths. All paths are relative to the app home and use forward slashes.
pub struct HomeLayout;

impl HomeLayout {
    /// `config/` — settings and updater state.
    pub const CONFIG_DIR: &'static str = "config";
    /// `config/settings.json` — application settings.
    pub const SETTINGS_FILE: &'static str = "config/settings.json";
    /// `config/update-channel.json` — updater state.
    pub const UPDATE_CHANNEL_FILE: &'static str = "config/update-channel.json";
    /// `data/` — the SQLite database.
    pub const DATA_DIR: &'static str = "data";
    /// `data/openconkit.sqlite3` — the application database.
    pub const DATABASE_FILE: &'static str = "data/openconkit.sqlite3";
    /// `projects/` — per-project files.
    pub const PROJECTS_DIR: &'static str = "projects";
    /// `codex-home/` — Codex sidecar home.
    pub const CODEX_HOME_DIR: &'static str = "codex-home";
    /// `codex-home/config.toml` — Codex sidecar configuration.
    pub const CODEX_CONFIG_FILE: &'static str = "codex-home/config.toml";
    /// `codex-home/log/` — Codex sidecar logs.
    pub const CODEX_LOG_DIR: &'static str = "codex-home/log";
    /// `ai-sandbox/` — sandbox for AI-produced artifacts.
    pub const AI_SANDBOX_DIR: &'static str = "ai-sandbox";
    /// `cache/` — disposable caches.
    pub const CACHE_DIR: &'static str = "cache";
    /// `logs/` — application logs (only when diagnostic logging is enabled).
    pub const LOGS_DIR: &'static str = "logs";
    /// `temp/` — scratch space for atomic writes.
    pub const TEMP_DIR: &'static str = "temp";
    /// `backups/` — backups of replaced/corrupt files.
    pub const BACKUPS_DIR: &'static str = "backups";

    /// `projects/<id>` — root directory of one project.
    pub fn project_dir(project_id: &ProjectId) -> String {
        format!("projects/{project_id}")
    }

    /// `projects/<id>/sources` — immutable imported source workbooks.
    pub fn project_sources_dir(project_id: &ProjectId) -> String {
        format!("{}/sources", Self::project_dir(project_id))
    }

    /// `projects/<id>/runs` — per-run artifacts.
    pub fn project_runs_dir(project_id: &ProjectId) -> String {
        format!("{}/runs", Self::project_dir(project_id))
    }

    /// `projects/<id>/exports` — generated reports.
    pub fn project_exports_dir(project_id: &ProjectId) -> String {
        format!("{}/exports", Self::project_dir(project_id))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    const ALL_CONSTANTS: [&str; 14] = [
        HomeLayout::CONFIG_DIR,
        HomeLayout::SETTINGS_FILE,
        HomeLayout::UPDATE_CHANNEL_FILE,
        HomeLayout::DATA_DIR,
        HomeLayout::DATABASE_FILE,
        HomeLayout::PROJECTS_DIR,
        HomeLayout::CODEX_HOME_DIR,
        HomeLayout::CODEX_CONFIG_FILE,
        HomeLayout::CODEX_LOG_DIR,
        HomeLayout::AI_SANDBOX_DIR,
        HomeLayout::CACHE_DIR,
        HomeLayout::LOGS_DIR,
        HomeLayout::TEMP_DIR,
        HomeLayout::BACKUPS_DIR,
    ];

    #[test]
    fn layout_constants_are_safe_relative_paths() {
        for path in ALL_CONSTANTS {
            assert!(!path.contains(".."), "{path} escapes the app home");
            assert!(!path.contains('\\'), "{path} must use forward slashes");
            assert!(!path.starts_with('/'), "{path} must be relative");
            assert!(
                !path.contains(':'),
                "{path} must not contain a drive/UNC prefix"
            );
        }
    }

    #[test]
    fn project_dirs_are_relative_and_forward_slashed() {
        let id = ProjectId::new("tower-a").expect("slug");
        for path in [
            HomeLayout::project_dir(&id),
            HomeLayout::project_sources_dir(&id),
            HomeLayout::project_runs_dir(&id),
            HomeLayout::project_exports_dir(&id),
        ] {
            assert!(path.starts_with("projects/tower-a"), "{path}");
            assert!(!path.contains(".."), "{path}");
            assert!(!path.contains('\\'), "{path}");
        }
        assert_eq!(
            HomeLayout::project_sources_dir(&id),
            "projects/tower-a/sources"
        );
        assert_eq!(HomeLayout::project_runs_dir(&id), "projects/tower-a/runs");
        assert_eq!(
            HomeLayout::project_exports_dir(&id),
            "projects/tower-a/exports"
        );
    }

    #[test]
    fn bootstrap_status_serializes() {
        let status = BootstrapStatus {
            home_path: "C:\\Users\\qs\\.openconkit".into(),
            created_fresh: true,
            structure_validated: true,
            recovered_from_interrupt: false,
            config_warnings: vec![],
            database_migrations: vec!["0001_initial".into()],
            backups_created: vec![],
        };
        let json = serde_json::to_string(&status).expect("serialize");
        let back: BootstrapStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, status);
    }
}
