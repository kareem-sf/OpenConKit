//! Process-wide application state managed by Tauri.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use openconkit_ai_codex::CodexCancellationToken;
use openconkit_application::{AppSettings, BootstrapStatus, UpdateChannelState};
use openconkit_domain::AnalysisRunId;
use openconkit_storage::Database;
use openconkit_tool_sdk::{CancellationToken, ToolRegistry};

use crate::codex::CodexRuntime;

/// Shared state available to every Tauri command.
pub struct AppState {
    /// Resolved app home directory.
    pub home: PathBuf,
    /// Bootstrap report from launch.
    pub bootstrap: BootstrapStatus,
    /// Open, migrated SQLite database.
    pub database: Arc<Database>,
    /// In-memory settings (persisted via [`openconkit_storage::SettingsStore`]).
    pub settings: Mutex<AppSettings>,
    /// In-memory updater state.
    pub update_channel: Mutex<UpdateChannelState>,
    /// Serializes update checks and installs so a package cannot race itself.
    pub updater_operation: tokio::sync::Mutex<()>,
    /// Compile-time tool registry.
    pub tools: Arc<ToolRegistry>,
    /// Cooperative cancellation tokens for currently executing runs.
    pub active_runs: Arc<Mutex<HashMap<AnalysisRunId, CancellationToken>>>,
    /// Lazy optional Codex runtime; uses an async lock because process
    /// initialization and shutdown are asynchronous.
    pub codex: tokio::sync::Mutex<CodexRuntime>,
    /// One paid AI turn per deterministic run at a time.
    pub active_ai_runs: Arc<Mutex<HashMap<AnalysisRunId, CodexCancellationToken>>>,
}

impl AppState {
    /// Borrow the settings mutex.
    pub fn settings(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, AppSettings>, crate::error::DesktopError> {
        self.settings
            .lock()
            .map_err(|_| crate::error::DesktopError::StatePoisoned)
    }

    /// Borrow the update-channel mutex.
    pub fn update_channel(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, UpdateChannelState>, crate::error::DesktopError> {
        self.update_channel
            .lock()
            .map_err(|_| crate::error::DesktopError::StatePoisoned)
    }

    pub fn active_runs(
        &self,
    ) -> Result<
        std::sync::MutexGuard<'_, HashMap<AnalysisRunId, CancellationToken>>,
        crate::error::DesktopError,
    > {
        self.active_runs
            .lock()
            .map_err(|_| crate::error::DesktopError::StatePoisoned)
    }

    pub fn active_ai_runs(
        &self,
    ) -> Result<
        std::sync::MutexGuard<'_, HashMap<AnalysisRunId, CodexCancellationToken>>,
        crate::error::DesktopError,
    > {
        self.active_ai_runs
            .lock()
            .map_err(|_| crate::error::DesktopError::StatePoisoned)
    }
}
