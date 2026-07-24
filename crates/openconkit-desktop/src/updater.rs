//! Rust-owned, explicitly invoked updater surface.
//!
//! The WebView never receives the updater plugin capability. It can only ask
//! these commands to check the two compiled-in HTTPS feeds, install a
//! version that is revalidated immediately before download, or open the
//! allowlisted project release page for a portable build.

use std::path::Path;
use std::time::Duration;

use jiff::Timestamp;
use openconkit_application::{
    AvailableUpdate, UpdateChannel, UpdateCheckResult, UpdateProgressEvent, UpdateProgressPhase,
};
use openconkit_storage::SettingsStore;
use semver::Version;
use serde_json::Value;
#[cfg(any(not(debug_assertions), test))]
use tauri::Manager;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use url::Url;

use crate::error::DesktopError;
use crate::state::AppState;

const STABLE_FEED: &str =
    "https://raw.githubusercontent.com/kareem-sf/OpenConKit/updates/latest-stable.json";
const BETA_FEED: &str =
    "https://raw.githubusercontent.com/kareem-sf/OpenConKit/updates/latest-beta.json";
const RELEASE_PAGE_PREFIX: &str = "https://github.com/kareem-sf/OpenConKit/releases/tag/v";
const PORTABLE_MARKER: &str = "OPENCONKIT_PORTABLE";
const UPDATE_PROGRESS_EVENT: &str = "update-progress";
#[cfg(any(not(debug_assertions), test))]
const UPDATE_AVAILABLE_EVENT: &str = "update-available";
const MAX_RELEASE_NOTES_CHARS: usize = 16_384;
const MAX_JSON_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);
#[cfg(any(not(debug_assertions), test))]
const AUTOMATIC_CHECK_DELAY: Duration = Duration::from_secs(15);
#[cfg(any(not(debug_assertions), test))]
const AUTOMATIC_CHECK_INTERVAL_SECONDS: i64 = 24 * 60 * 60;

/// Start a non-blocking, best-effort update check for release builds. Network
/// or feed errors are intentionally silent; the manual command reports them.
#[cfg(any(not(debug_assertions), test))]
pub fn schedule_automatic_check(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(AUTOMATIC_CHECK_DELAY).await;
        let _ = automatic_check(app).await;
    });
}

/// Check the selected, compiled-in update feed and persist a successful check
/// timestamp. Merely opening the application never calls this command.
#[tauri::command]
pub async fn check_for_updates(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UpdateCheckResult, DesktopError> {
    let _operation = state.updater_operation.lock().await;
    let channel = state.update_channel()?.channel;
    let update = check_channel(&app, channel).await?;
    let checked_at = Timestamp::now();
    persist_successful_check(&state, checked_at)?;
    make_check_result(&app, channel, checked_at, update)
}

/// Recheck the feed, require the exact version shown to the user, download,
/// verify the Tauri signature, and launch the platform installer.
#[tauri::command(rename_all = "snake_case")]
pub async fn install_update(
    app: AppHandle,
    state: State<'_, AppState>,
    expected_version: String,
    channel: UpdateChannel,
) -> Result<(), DesktopError> {
    let _operation = state.updater_operation.lock().await;
    let selected_channel = state.update_channel()?.channel;
    if selected_channel != channel {
        return Err(coded(
            "UPDATE_CHANNEL_CHANGED",
            "the selected update channel changed before installation",
        ));
    }
    validate_channel_version(channel, &expected_version)?;
    if is_portable_executable(&std::env::current_exe().map_err(update_io_error)?) {
        return Err(coded(
            "UPDATE_PORTABLE_MANUAL",
            "portable builds cannot be updated in place",
        ));
    }

    let Some(update) = check_channel(&app, channel).await? else {
        return Err(coded(
            "UPDATE_NOT_AVAILABLE",
            "the selected feed no longer announces an update",
        ));
    };
    let checked_at = Timestamp::now();
    persist_successful_check(&state, checked_at)?;
    if update.version != expected_version {
        return Err(coded(
            "UPDATE_CHANGED",
            "the feed version changed after the user reviewed it",
        ));
    }

    let version = update.version.clone();
    let progress_app = app.clone();
    let chunk_version = version.clone();
    let mut downloaded_bytes = 0_u64;
    let download = update.download(
        move |chunk, total| {
            downloaded_bytes = downloaded_bytes.saturating_add(chunk as u64);
            emit_progress(
                &progress_app,
                &chunk_version,
                UpdateProgressPhase::Downloading,
                downloaded_bytes,
                total,
            );
        },
        || {},
    );
    let bytes = tokio::time::timeout(DOWNLOAD_TIMEOUT, download)
        .await
        .map_err(|_| coded("UPDATE_DOWNLOAD_TIMEOUT", "update download timed out"))?
        .map_err(map_updater_error)?;

    emit_progress(
        &app,
        &version,
        UpdateProgressPhase::Downloaded,
        bytes.len() as u64,
        Some(bytes.len() as u64),
    );
    emit_progress(
        &app,
        &version,
        UpdateProgressPhase::Installing,
        bytes.len() as u64,
        Some(bytes.len() as u64),
    );
    update.install(bytes).map_err(map_updater_error)
}

/// Open a project-owned GitHub release URL derived from a validated semantic
/// version. No feed-provided URL crosses into the opener.
#[tauri::command(rename_all = "snake_case")]
pub fn open_update_download(expected_version: String) -> Result<(), DesktopError> {
    let url = manual_download_url(&expected_version)?;
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|error| {
        coded(
            "UPDATE_BROWSER_OPEN_FAILED",
            format!("failed to open release page: {error}"),
        )
    })
}

#[cfg(any(not(debug_assertions), test))]
async fn automatic_check(app: AppHandle) -> Result<(), DesktopError> {
    let state = app.state::<AppState>();
    let _operation = state.updater_operation.lock().await;
    let (channel, last_check) = {
        let channel_state = state.update_channel()?;
        (
            channel_state.channel,
            channel_state.last_successful_update_check,
        )
    };
    let now = Timestamp::now();
    if !automatic_check_due(now, last_check) {
        return Ok(());
    }
    let update = check_channel(&app, channel).await?;
    let checked_at = Timestamp::now();
    persist_successful_check(&state, checked_at)?;
    let result = make_check_result(&app, channel, checked_at, update)?;
    if result.update.is_some() {
        let _ = app.emit(UPDATE_AVAILABLE_EVENT, result);
    }
    Ok(())
}

async fn check_channel(
    app: &AppHandle,
    channel: UpdateChannel,
) -> Result<Option<Update>, DesktopError> {
    let endpoint = Url::parse(feed_for(channel)).map_err(|error| {
        coded(
            "UPDATE_CONFIGURATION_INVALID",
            format!("compiled-in update endpoint is invalid: {error}"),
        )
    })?;
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(map_updater_error)?
        .timeout(CHECK_TIMEOUT)
        .build()
        .map_err(map_updater_error)?;
    let update = updater.check().await.map_err(map_updater_error)?;
    if let Some(candidate) = update.as_ref() {
        validate_channel_version(channel, &candidate.version)?;
    }
    Ok(update)
}

fn make_check_result(
    app: &AppHandle,
    channel: UpdateChannel,
    checked_at: Timestamp,
    update: Option<Update>,
) -> Result<UpdateCheckResult, DesktopError> {
    let portable = is_portable_executable(&std::env::current_exe().map_err(update_io_error)?);
    let current_version = app.package_info().version.to_string();
    let update = update
        .map(|candidate| {
            let version = candidate.version.clone();
            let notes = candidate.body.as_deref().map(bounded_release_notes);
            let size_bytes = update_size(&candidate.raw_json, candidate.download_url.as_str());
            let published_at = candidate.date.map(|date| date.to_string());
            Ok::<AvailableUpdate, DesktopError>(AvailableUpdate {
                manual_download_url: manual_download_url(&version)?,
                version,
                notes,
                published_at,
                size_bytes,
                can_install: !portable,
            })
        })
        .transpose()?;
    Ok(UpdateCheckResult {
        checked_at,
        channel,
        current_version,
        portable,
        update,
    })
}

fn persist_successful_check(
    state: &State<'_, AppState>,
    checked_at: Timestamp,
) -> Result<(), DesktopError> {
    let mut settings = state.settings()?;
    let mut channel_state = state.update_channel()?;
    let previous_channel_state = channel_state.clone();
    let mut next_settings = settings.clone();
    let mut next_channel_state = channel_state.clone();
    next_settings.last_successful_update_check = Some(checked_at);
    next_channel_state.last_successful_update_check = Some(checked_at);

    let store = SettingsStore::new(&state.home);
    store.save_update_channel(&next_channel_state)?;
    if let Err(error) = store.save_settings(&next_settings) {
        if let Err(rollback) = store.save_update_channel(&previous_channel_state) {
            return Err(DesktopError::Storage(format!(
                "{error}; update timestamp rollback also failed: {rollback}"
            )));
        }
        return Err(error.into());
    }
    *settings = next_settings;
    *channel_state = next_channel_state;
    Ok(())
}

fn feed_for(channel: UpdateChannel) -> &'static str {
    match channel {
        UpdateChannel::Stable => STABLE_FEED,
        UpdateChannel::Beta => BETA_FEED,
    }
}

fn validate_channel_version(
    channel: UpdateChannel,
    version: &str,
) -> Result<Version, DesktopError> {
    let parsed = Version::parse(version)
        .map_err(|error| coded("UPDATE_FEED_INVALID", format!("invalid version: {error}")))?;
    if channel == UpdateChannel::Stable && !parsed.pre.is_empty() {
        return Err(coded(
            "UPDATE_FEED_INVALID",
            "stable update feed announced a pre-release",
        ));
    }
    Ok(parsed)
}

fn manual_download_url(version: &str) -> Result<String, DesktopError> {
    let version = Version::parse(version)
        .map_err(|error| coded("UPDATE_FEED_INVALID", format!("invalid version: {error}")))?;
    Ok(format!("{RELEASE_PAGE_PREFIX}{version}"))
}

fn bounded_release_notes(notes: &str) -> String {
    notes.chars().take(MAX_RELEASE_NOTES_CHARS).collect()
}

fn update_size(raw: &Value, selected_url: &str) -> Option<u64> {
    raw.get("size").and_then(json_u64).or_else(|| {
        raw.get("platforms")?
            .as_object()?
            .values()
            .find(|platform| platform.get("url").and_then(Value::as_str) == Some(selected_url))
            .and_then(|platform| platform.get("size"))
            .and_then(json_u64)
    })
}

fn json_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .filter(|size| *size <= MAX_JSON_SAFE_INTEGER)
}

fn is_portable_executable(executable: &Path) -> bool {
    executable
        .parent()
        .is_some_and(|parent| parent.join(PORTABLE_MARKER).is_file())
}

#[cfg(any(not(debug_assertions), test))]
fn automatic_check_due(now: Timestamp, last_check: Option<Timestamp>) -> bool {
    match last_check {
        None => true,
        Some(last_check) if last_check > now => true,
        Some(last_check) => {
            now.as_second().saturating_sub(last_check.as_second())
                >= AUTOMATIC_CHECK_INTERVAL_SECONDS
        }
    }
}

fn emit_progress(
    app: &AppHandle,
    version: &str,
    phase: UpdateProgressPhase,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) {
    let _ = app.emit(
        UPDATE_PROGRESS_EVENT,
        UpdateProgressEvent {
            version: version.to_string(),
            phase,
            downloaded_bytes,
            total_bytes,
        },
    );
}

fn map_updater_error(error: tauri_plugin_updater::Error) -> DesktopError {
    use tauri_plugin_updater::Error;
    let code = match &error {
        Error::Reqwest(_) | Error::Network(_) | Error::ReleaseNotFound => "UPDATE_OFFLINE",
        Error::Minisign(_) | Error::Base64(_) | Error::SignatureUtf8(_) => {
            "UPDATE_SIGNATURE_INVALID"
        }
        Error::UnsupportedArch
        | Error::UnsupportedOs
        | Error::TargetNotFound(_)
        | Error::TargetsNotFound(_) => "UPDATE_PLATFORM_UNSUPPORTED",
        Error::Semver(_)
        | Error::Serialization(_)
        | Error::UrlParse(_)
        | Error::EmptyEndpoints
        | Error::InsecureTransportProtocol => "UPDATE_FEED_INVALID",
        _ => "UPDATE_INSTALL_FAILED",
    };
    coded(code, error.to_string())
}

fn update_io_error(error: std::io::Error) -> DesktopError {
    coded(
        "UPDATE_INSTALL_FAILED",
        format!("could not locate the running executable: {error}"),
    )
}

fn coded(code: &'static str, message: impl Into<String>) -> DesktopError {
    DesktopError::Coded {
        code,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::*;

    #[test]
    fn automatic_scheduler_is_compiled_in_test_builds() {
        let _scheduler: fn(AppHandle) = schedule_automatic_check;
    }

    #[test]
    fn feeds_are_fixed_https_project_urls() {
        for channel in [UpdateChannel::Stable, UpdateChannel::Beta] {
            let url = Url::parse(feed_for(channel)).expect("valid URL");
            assert_eq!(url.scheme(), "https");
            assert_eq!(url.host_str(), Some("raw.githubusercontent.com"));
            assert!(url.path().starts_with("/kareem-sf/OpenConKit/updates/"));
        }
    }

    #[test]
    fn stable_feed_rejects_prerelease() {
        assert!(validate_channel_version(UpdateChannel::Stable, "1.2.3").is_ok());
        assert_eq!(
            validate_channel_version(UpdateChannel::Stable, "1.2.3-beta.1")
                .expect_err("must reject")
                .code(),
            "UPDATE_FEED_INVALID"
        );
        assert!(validate_channel_version(UpdateChannel::Beta, "1.2.3-beta.1").is_ok());
    }

    #[test]
    fn manual_url_cannot_be_redirected_by_version_input() {
        assert_eq!(
            manual_download_url("1.2.3").expect("valid"),
            "https://github.com/kareem-sf/OpenConKit/releases/tag/v1.2.3"
        );
        assert!(manual_download_url("1.2.3/../../attacker").is_err());
    }

    #[test]
    fn selected_platform_size_is_extracted_by_exact_url() {
        let raw = json!({
            "platforms": {
                "windows-x86_64-nsis": {
                    "url": "https://github.com/kareem-sf/OpenConKit/releases/download/v1/app.nsis.zip",
                    "signature": "signed",
                    "size": "12345"
                },
                "linux-x86_64-appimage": {
                    "url": "https://github.com/kareem-sf/OpenConKit/releases/download/v1/app.AppImage.tar.gz",
                    "signature": "signed",
                    "size": 99
                }
            }
        });
        assert_eq!(
            update_size(
                &raw,
                "https://github.com/kareem-sf/OpenConKit/releases/download/v1/app.nsis.zip"
            ),
            Some(12_345)
        );
    }

    #[test]
    fn portable_marker_must_be_next_to_executable() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("openconkit-portable-{nanos}"));
        fs::create_dir_all(&directory).expect("mkdir");
        let executable = directory.join("OpenConKit.exe");
        assert!(!is_portable_executable(&executable));
        fs::write(directory.join(PORTABLE_MARKER), b"portable\n").expect("marker");
        assert!(is_portable_executable(&executable));
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn release_notes_are_bounded_on_unicode_scalar_boundaries() {
        let notes = "é".repeat(MAX_RELEASE_NOTES_CHARS + 5);
        let bounded = bounded_release_notes(&notes);
        assert_eq!(bounded.chars().count(), MAX_RELEASE_NOTES_CHARS);
        assert!(bounded.is_char_boundary(bounded.len()));
    }

    #[test]
    fn automatic_check_interval_handles_missing_stale_recent_and_future_values() {
        let now: Timestamp = "2026-07-24T08:00:00Z".parse().expect("timestamp");
        let stale: Timestamp = "2026-07-23T07:59:59Z".parse().expect("timestamp");
        let recent: Timestamp = "2026-07-24T07:59:59Z".parse().expect("timestamp");
        let future: Timestamp = "2026-07-25T08:00:00Z".parse().expect("timestamp");
        assert!(automatic_check_due(now, None));
        assert!(automatic_check_due(now, Some(stale)));
        assert!(!automatic_check_due(now, Some(recent)));
        assert!(automatic_check_due(now, Some(future)));
    }
}
