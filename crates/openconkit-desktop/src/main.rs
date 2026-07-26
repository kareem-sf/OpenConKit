// Tauri entry point. Release builds use the Windows GUI subsystem
// (no console window); debug builds keep the console for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod codex;
mod commands;
mod error;
mod registry;
mod state;
mod updater;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use openconkit_storage::{bootstrap_home, resolve_home};
use tauri::Manager;

use crate::codex::CodexRuntime;
use crate::error::DesktopError;
use crate::registry::build_registry;
use crate::state::AppState;

/// Build and run the Tauri application.
fn main() {
    tauri::Builder::default()
        // Must be the first plugin: a second process must exit before setup
        // can open or migrate the shared application database.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let resource_directory = app.path().resource_dir().ok();
            let state = bootstrap_app_state(resource_directory.as_deref()).map_err(|err| {
                eprintln!("fatal: bootstrap failed: {err}");
                err
            })?;
            let webview_data_directory = webview_data_directory(&state.home);
            app.manage(state);
            create_main_window(app, &webview_data_directory)?;
            #[cfg(not(debug_assertions))]
            updater::schedule_automatic_check(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::openconkit_home,
            commands::bootstrap_status,
            commands::get_settings,
            commands::update_settings,
            commands::reset_openconkit,
            updater::check_for_updates,
            updater::install_update,
            updater::open_update_download,
            commands::ai_runtime_status,
            commands::get_ai_account,
            commands::start_ai_login,
            commands::cancel_ai_login,
            commands::logout_ai,
            commands::get_ai_rate_limits,
            commands::prepare_ai_review,
            commands::run_ai_review,
            commands::cancel_ai_review,
            commands::list_storage_groups,
            commands::quick_import_source,
            commands::list_source_revisions,
            commands::run_tool,
            commands::cancel_tool_run,
            commands::list_analysis_runs,
            commands::list_run_history,
            commands::open_analysis_run,
            commands::export_analysis_run,
            commands::list_run_exports,
            commands::reveal_export,
            commands::list_tool_manifests,
            commands::list_tool_navigation,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|err| {
            // The Tauri runtime failing to start is unrecoverable. Release
            // builds have no console, so Windows also needs an actionable
            // native message (notably for portable copies missing WebView2).
            #[cfg(target_os = "windows")]
            show_windows_startup_failure(&err);
            eprintln!("fatal: failed to start OpenConKit: {err}");
            std::process::exit(1);
        });
}

/// Keep native webview caches inside the canonical app home.
///
/// The main window is disabled in static configuration and created here only
/// after app-home resolution. This avoids WebView2's executable-adjacent
/// fallback on Windows and gives Linux WebKit the same bounded cache root.
/// macOS uses the configured non-persistent data store because WKWebView does
/// not accept a custom data directory.
fn create_main_window(app: &mut tauri::App, data_directory: &Path) -> Result<(), DesktopError> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
        .ok_or_else(|| {
            DesktopError::WindowStartup("main window configuration is missing".to_string())
        })?;

    tauri::WebviewWindowBuilder::from_config(app.handle(), config)
        .map_err(|err| DesktopError::WindowStartup(err.to_string()))?
        .data_directory(data_directory.to_path_buf())
        .build()
        .map_err(|err| DesktopError::WindowStartup(err.to_string()))?;
    Ok(())
}

fn webview_data_directory(home: &Path) -> PathBuf {
    home.join("cache").join("webview")
}

#[cfg(target_os = "windows")]
fn show_windows_startup_failure(error: &tauri::Error) {
    use rfd::{MessageButtons, MessageDialog, MessageLevel};

    let diagnostic = error.to_string().to_ascii_lowercase();
    let message_key = if diagnostic.contains("webview") || diagnostic.contains("webview2") {
        "webviewMissing"
    } else {
        "genericFailure"
    };
    let title = bilingual_startup_copy("title");
    let description = bilingual_startup_copy(message_key);
    let _ = MessageDialog::new()
        .set_level(MessageLevel::Error)
        .set_title(&title)
        .set_description(&description)
        .set_buttons(MessageButtons::Ok)
        .show();
}

#[cfg(target_os = "windows")]
fn bilingual_startup_copy(key: &str) -> String {
    const EN: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/i18n/src/locales/en/common.json"
    ));
    const AR: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/i18n/src/locales/ar/common.json"
    ));

    [EN, AR]
        .iter()
        .filter_map(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .filter_map(|locale| {
            locale
                .get("startup")
                .and_then(|startup| startup.get(key))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Resolve home, bootstrap layout/DB/settings, and compose the tool registry.
fn bootstrap_app_state(
    resource_directory: Option<&std::path::Path>,
) -> Result<AppState, DesktopError> {
    let home = resolve_home().map_err(|err| match err {
        openconkit_storage::StorageError::HomeOverrideEmpty => DesktopError::HomeOverrideEmpty,
        openconkit_storage::StorageError::HomeOverrideNotAllowed => {
            DesktopError::HomeOverrideNotAllowed
        }
        openconkit_storage::StorageError::HomeNotFound => DesktopError::HomeNotFound,
        other => DesktopError::Bootstrap(other.to_string()),
    })?;

    let result = bootstrap_home(&home).map_err(|err| DesktopError::Bootstrap(err.to_string()))?;
    let tools = build_registry()?;

    let system_codex_binary = result
        .settings
        .advanced
        .use_system_codex
        .then_some(result.settings.advanced.system_codex_binary.as_deref())
        .flatten();
    let codex = CodexRuntime::new(
        &home,
        resource_directory,
        system_codex_binary,
        result.settings.privacy.diagnostic_logging_enabled,
    );
    Ok(AppState {
        home,
        bootstrap: result.status,
        database: Arc::new(result.database),
        settings: Mutex::new(result.settings),
        update_channel: Mutex::new(result.update_channel),
        updater_operation: tokio::sync::Mutex::new(()),
        tools: Arc::new(tools),
        active_runs: Arc::new(Mutex::new(HashMap::new())),
        codex: tokio::sync::Mutex::new(codex),
        active_ai_runs: Arc::new(Mutex::new(HashMap::new())),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use openconkit_application::{ListProjects, RegisterProject};
    use openconkit_storage::SqliteProjectRepository;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn bootstrap_app_state_against_temp_home() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let home = std::env::temp_dir().join(format!("openconkit-desktop-boot-{nanos}"));
        let previous = std::env::var_os("OPENCONKIT_HOME");
        std::env::set_var("OPENCONKIT_HOME", &home);

        let state = bootstrap_app_state(None).expect("bootstrap");
        assert!(state.bootstrap.structure_validated);
        assert_eq!(state.tools.len(), 1);
        assert!(state.tools.get("boq-inspector").is_some());

        let repo = SqliteProjectRepository::new(&state.database);
        let project = RegisterProject::new(&repo)
            .execute("tower-a", "Tower A")
            .expect("register");
        assert_eq!(project.name(), "Tower A");
        let listed = ListProjects::new(&repo).execute(false).expect("list");
        assert_eq!(listed.len(), 1);

        // Restore env.
        std::env::remove_var("OPENCONKIT_HOME");
        if let Some(value) = previous {
            std::env::set_var("OPENCONKIT_HOME", value);
        }
        let _ = std::fs::remove_dir_all(&home);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn native_startup_copy_comes_from_both_locale_files() {
        let title = bilingual_startup_copy("title");
        let message = bilingual_startup_copy("webviewMissing");
        assert!(title.contains("OpenConKit"));
        assert!(title.contains("تعذّر"));
        assert!(message.contains("WebView2"));
        assert!(message.contains("Microsoft"));
    }

    #[test]
    fn webview_data_stays_inside_app_home() {
        let home = std::path::PathBuf::from("openconkit-home");
        assert_eq!(
            webview_data_directory(&home),
            home.join("cache").join("webview")
        );
    }
}
