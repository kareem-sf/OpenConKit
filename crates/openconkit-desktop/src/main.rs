// Tauri entry point. Release builds use the Windows GUI subsystem
// (no console window); debug builds keep the console for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod commands;
mod error;

/// Build and run the Tauri application.
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::openconkit_home,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|err| {
            // The Tauri runtime failing to start is unrecoverable: report and exit.
            eprintln!("fatal: failed to start OpenConKit: {err}");
            std::process::exit(1);
        });
}
