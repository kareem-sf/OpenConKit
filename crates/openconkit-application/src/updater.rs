//! Pure updater DTOs shared by the desktop host and TypeScript frontend.
//!
//! Network access and package installation remain infrastructure concerns
//! owned by the Tauri host. These types only describe the validated state
//! that may cross the IPC boundary.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::UpdateChannel;

/// A newer, platform-compatible release announced by the selected feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AvailableUpdate {
    /// Semantic version announced by the signed-release feed.
    pub version: String,
    /// Plain-text release notes, bounded by the desktop host.
    pub notes: Option<String>,
    /// RFC 3339 publication timestamp from the feed.
    pub published_at: Option<String>,
    /// Installer size from the selected platform entry, when supplied.
    #[ts(type = "number | null")]
    pub size_bytes: Option<u64>,
    /// Whether this package can be installed in place.
    pub can_install: bool,
    /// Allowlisted browser URL for a manual download.
    pub manual_download_url: String,
}

/// Result of one explicit update check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UpdateCheckResult {
    /// Time at which a valid response was received.
    pub checked_at: Timestamp,
    /// Feed channel used by this request.
    pub channel: UpdateChannel,
    /// Currently running application version.
    pub current_version: String,
    /// Whether this executable was distributed as a portable package.
    pub portable: bool,
    /// New release metadata, or `None` when already current.
    pub update: Option<AvailableUpdate>,
}

/// Updater lifecycle phase emitted while an explicit install is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum UpdateProgressPhase {
    /// Bytes are being downloaded.
    Downloading,
    /// Download completed and its signature was verified.
    Downloaded,
    /// The platform installer is being launched.
    Installing,
}

/// Progress event for one update version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UpdateProgressEvent {
    /// Version being installed.
    pub version: String,
    /// Current updater phase.
    pub phase: UpdateProgressPhase,
    /// Cumulative bytes received.
    #[ts(type = "number")]
    pub downloaded_bytes: u64,
    /// Server-provided content length, when available.
    #[ts(type = "number | null")]
    pub total_bytes: Option<u64>,
}
