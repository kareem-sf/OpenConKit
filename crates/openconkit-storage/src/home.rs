//! App-home bootstrap: create/validate layout, load config, migrate DB.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use jiff::Timestamp;
use openconkit_application::{AppSettings, BootstrapStatus, HomeLayout, UpdateChannelState};

use crate::atomic::unique_temp_suffix;
use crate::database::Database;
use crate::migrations::MIGRATIONS;
use crate::permissions::{harden_directory, harden_file, make_owner_writable};
use crate::settings_store::SettingsStore;
use crate::StorageError;

/// Marker file written for the duration of first-launch setup so a crash
/// mid-bootstrap can be detected and recovered from on the next launch.
const INTERRUPT_MARKER: &str = "temp/.bootstrap-in-progress";

/// Marker consumed on the next launch before app-home bootstrap.
const FACTORY_RESET_MARKER: &str = ".factory-reset-requested";

/// Minimal Codex profile: OS credential store preferred, no telemetry, no
/// self-update (OpenConKit pins the sidecar version). Expanded in Phase 7.
const CODEX_CONFIG_TOML: &str = r#"# Managed by OpenConKit. Do not edit by hand unless you know what you are doing.
# OpenConKit pins the Codex sidecar version; disable Codex's own updater.
# Prefer the OS credential store for auth tokens.
cli_auth_credentials_store = "auto"
"#;

/// Result of bootstrapping the app home.
pub struct BootstrapResult {
    /// Status reported to the frontend.
    pub status: BootstrapStatus,
    /// Open, migrated database handle.
    pub database: Database,
    /// Loaded (or defaulted) application settings.
    pub settings: AppSettings,
    /// Loaded (or defaulted) updater state.
    pub update_channel: UpdateChannelState,
}

/// Bootstrap the app home at `home`.
///
/// Steps:
/// 1. Detect a previous interrupted bootstrap and clean the marker.
/// 2. Create the canonical directory tree (idempotent).
/// 3. Write default config files if missing; load with fail-safe recovery.
/// 4. Write a minimal Codex profile if missing.
/// 5. Open and migrate the SQLite database.
/// 6. Clear the interrupt marker and return the status + handles.
pub fn bootstrap_home(home: &Path) -> Result<BootstrapResult, StorageError> {
    apply_pending_factory_reset(home)?;
    let created_fresh = !home.exists();
    let interrupt_marker = home.join(path_from_rel(INTERRUPT_MARKER));
    let recovered_from_interrupt = interrupt_marker.exists();

    fs::create_dir_all(home).map_err(io_err)?;
    harden_directory(home).map_err(io_err)?;
    // Mark bootstrap in progress before any further mutation.
    if let Some(parent) = interrupt_marker.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    write_marker(&interrupt_marker)?;

    create_layout(home)?;

    let store = SettingsStore::new(home);
    store.ensure_defaults()?;

    let mut config_warnings = Vec::new();
    let mut backups_created = Vec::new();

    let (settings, settings_warnings, settings_backups) = store.load_settings()?;
    config_warnings.extend(settings_warnings);
    backups_created.extend(settings_backups);

    let (mut update_channel, channel_warnings, channel_backups) = store.load_update_channel()?;
    config_warnings.extend(channel_warnings);
    backups_created.extend(channel_backups);
    if update_channel.channel != settings.update_channel
        || update_channel.last_successful_update_check != settings.last_successful_update_check
    {
        update_channel.channel = settings.update_channel;
        update_channel.last_successful_update_check = settings.last_successful_update_check;
        store.save_update_channel(&update_channel)?;
        config_warnings.push(
            "update-channel state was reconciled from canonical application settings".to_string(),
        );
    }

    ensure_codex_config(home)?;

    let db_path = home.join(path_from_rel(HomeLayout::DATABASE_FILE));
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
        harden_directory(parent).map_err(io_err)?;
    }
    let database_existed = db_path.is_file();
    let database = Database::open(&db_path)?;
    let version_before = database.schema_version()?;
    let latest = MIGRATIONS.last().map_or(0, |migration| migration.version);
    if database_existed && version_before < latest {
        backups_created.push(backup_database(home, &database, version_before, latest)?);
    }
    database.migrate()?;
    let version_after = database.schema_version()?;
    let database_migrations: Vec<String> = MIGRATIONS
        .iter()
        .filter(|m| m.version > version_before && m.version <= version_after)
        .map(|m| format!("{:04}_{}", m.version, m.description.replace(' ', "_")))
        .collect();

    // Bootstrap finished cleanly.
    let _ = fs::remove_file(&interrupt_marker);

    let status = BootstrapStatus {
        home_path: home.to_string_lossy().into_owned(),
        created_fresh,
        structure_validated: true,
        recovered_from_interrupt,
        config_warnings,
        database_migrations,
        backups_created,
    };

    Ok(BootstrapResult {
        status,
        database,
        settings,
        update_channel,
    })
}

/// Schedule deletion of the canonical app home on the next launch.
///
/// The desktop host restarts immediately after writing this marker so the
/// open SQLite and WebView handles are released before deletion is attempted.
pub fn request_factory_reset(home: &Path) -> Result<(), StorageError> {
    validate_factory_reset_target(home)?;
    let marker = home.join(FACTORY_RESET_MARKER);
    if marker.exists() {
        let metadata = fs::symlink_metadata(&marker).map_err(io_err)?;
        if metadata.file_type().is_file() && !metadata.file_type().is_symlink() {
            return Ok(());
        }
        return Err(StorageError::UnsafeFactoryResetTarget);
    }
    crate::atomic::atomic_write(&marker, b"reset-requested\n").map_err(io_err)
}

fn apply_pending_factory_reset(home: &Path) -> Result<(), StorageError> {
    let marker = home.join(FACTORY_RESET_MARKER);
    let marker_metadata = match fs::symlink_metadata(&marker) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_err(error)),
    };
    if !marker_metadata.file_type().is_file() || marker_metadata.file_type().is_symlink() {
        return Err(StorageError::UnsafeFactoryResetTarget);
    }
    validate_factory_reset_target(home)?;
    make_tree_removable(home)?;

    if let Err(error) = fs::remove_dir_all(home) {
        // Keep the reset request durable if an external process temporarily
        // holds a file open. The next launch will retry instead of silently
        // starting against a partially deleted app home.
        let _ = crate::atomic::atomic_write(&marker, b"reset-requested\n");
        return Err(io_err(error));
    }
    Ok(())
}

fn make_tree_removable(directory: &Path) -> Result<(), StorageError> {
    for entry in fs::read_dir(directory).map_err(io_err)? {
        let path = entry.map_err(io_err)?.path();
        let metadata = fs::symlink_metadata(&path).map_err(io_err)?;
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            make_tree_removable(&path)?;
        } else if file_type.is_file() {
            make_owner_writable(&path).map_err(io_err)?;
        } else {
            return Err(StorageError::UnsafeFactoryResetTarget);
        }
    }
    Ok(())
}

fn validate_factory_reset_target(home: &Path) -> Result<(), StorageError> {
    if !home.is_absolute() || home.parent().is_none() || home.file_name().is_none() {
        return Err(StorageError::UnsafeFactoryResetTarget);
    }
    let metadata = fs::symlink_metadata(home).map_err(io_err)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(StorageError::UnsafeFactoryResetTarget);
    }
    Ok(())
}

/// Create every top-level directory from [`HomeLayout`].
fn create_layout(home: &Path) -> Result<(), StorageError> {
    let dirs = [
        HomeLayout::CONFIG_DIR,
        HomeLayout::DATA_DIR,
        HomeLayout::PROJECTS_DIR,
        HomeLayout::CODEX_HOME_DIR,
        HomeLayout::CODEX_LOG_DIR,
        HomeLayout::AI_SANDBOX_DIR,
        HomeLayout::CACHE_DIR,
        HomeLayout::LOGS_DIR,
        HomeLayout::TEMP_DIR,
        HomeLayout::BACKUPS_DIR,
    ];
    for rel in dirs {
        let path = home.join(path_from_rel(rel));
        fs::create_dir_all(&path).map_err(io_err)?;
        harden_directory(&path).map_err(io_err)?;
    }
    Ok(())
}

fn backup_database(
    home: &Path,
    database: &Database,
    version_before: u32,
    version_after: u32,
) -> Result<String, StorageError> {
    let stamp = Timestamp::now().strftime("%Y%m%dT%H%M%SZ").to_string();
    let name = format!(
        "openconkit-db-v{version_before}-to-v{version_after}-{stamp}-{}.sqlite3",
        unique_temp_suffix()
    );
    let relative = format!("{}/{}", HomeLayout::BACKUPS_DIR, name);
    let path = home.join(path_from_rel(&relative));
    database.backup_to(&path)?;
    Ok(relative)
}

fn ensure_codex_config(home: &Path) -> Result<(), StorageError> {
    let path = home.join(path_from_rel(HomeLayout::CODEX_CONFIG_FILE));
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    crate::atomic::atomic_write(&path, CODEX_CONFIG_TOML.as_bytes()).map_err(io_err)?;
    Ok(())
}

fn write_marker(path: &Path) -> Result<(), StorageError> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(io_err)?;
    harden_file(path).map_err(io_err)?;
    file.write_all(b"in-progress\n").map_err(io_err)?;
    file.sync_all().map_err(io_err)?;
    Ok(())
}

fn path_from_rel(rel: &str) -> PathBuf {
    rel.split('/').collect()
}

fn io_err(err: std::io::Error) -> StorageError {
    StorageError::Io {
        message: err.to_string(),
    }
}

/// Resolve the canonical app home path.
///
/// Precedence: `OPENCONKIT_HOME` (dev/test override), then
/// `%USERPROFILE%\.openconkit` / `$HOME/.openconkit`.
pub fn resolve_home() -> Result<PathBuf, StorageError> {
    resolve_home_from(
        std::env::var_os("OPENCONKIT_HOME"),
        std::env::var_os("USERPROFILE"),
        std::env::var_os("HOME"),
        cfg!(debug_assertions) || cfg!(test),
    )
}

fn resolve_home_from(
    override_dir: Option<std::ffi::OsString>,
    user_profile: Option<std::ffi::OsString>,
    unix_home: Option<std::ffi::OsString>,
    allow_override: bool,
) -> Result<PathBuf, StorageError> {
    if let Some(override_dir) = override_dir {
        if !allow_override {
            return Err(StorageError::HomeOverrideNotAllowed);
        }
        if override_dir.is_empty() {
            return Err(StorageError::HomeOverrideEmpty);
        }
        return Ok(PathBuf::from(override_dir));
    }
    user_profile
        .or(unix_home)
        .map(|home| PathBuf::from(home).join(".openconkit"))
        .ok_or(StorageError::HomeNotFound)
}

// Silence unused import of File when only OpenOptions is used on some platforms.
#[allow(dead_code)]
fn _touch(path: &Path) -> Result<(), StorageError> {
    File::create(path).map_err(io_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use openconkit_application::ProjectRepository;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_home() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("openconkit-bootstrap-{nanos}"));
        // Do not create — bootstrap should create it.
        dir
    }

    #[test]
    fn first_launch_creates_full_layout_and_defaults() {
        let home = temp_home();
        let result = bootstrap_home(&home).expect("bootstrap");
        assert!(result.status.created_fresh);
        assert!(result.status.structure_validated);
        assert!(!result.status.recovered_from_interrupt);
        assert!(result.status.config_warnings.is_empty());
        assert!(!result.status.database_migrations.is_empty());
        assert!(result.status.database_migrations[0].contains("initial"));

        for rel in [
            HomeLayout::CONFIG_DIR,
            HomeLayout::DATA_DIR,
            HomeLayout::PROJECTS_DIR,
            HomeLayout::CODEX_HOME_DIR,
            HomeLayout::CODEX_LOG_DIR,
            HomeLayout::AI_SANDBOX_DIR,
            HomeLayout::CACHE_DIR,
            HomeLayout::LOGS_DIR,
            HomeLayout::TEMP_DIR,
            HomeLayout::BACKUPS_DIR,
            HomeLayout::SETTINGS_FILE,
            HomeLayout::UPDATE_CHANNEL_FILE,
            HomeLayout::DATABASE_FILE,
            HomeLayout::CODEX_CONFIG_FILE,
        ] {
            let path = home.join(path_from_rel(rel));
            assert!(path.exists(), "missing {rel} at {}", path.display());
        }

        // Interrupt marker must be gone after a clean bootstrap.
        assert!(!home.join(path_from_rel(INTERRUPT_MARKER)).exists());

        // DB is usable.
        let projects = crate::SqliteProjectRepository::new(&result.database);
        assert!(projects.list(true).expect("list").is_empty());

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn second_launch_is_idempotent_and_not_fresh() {
        let home = temp_home();
        let first = bootstrap_home(&home).expect("first");
        assert!(first.status.created_fresh);
        let migrations_first = first.status.database_migrations.clone();
        drop(first);

        let second = bootstrap_home(&home).expect("second");
        assert!(!second.status.created_fresh);
        assert!(second.status.database_migrations.is_empty());
        assert!(second.status.structure_validated);
        // Defaults still present.
        assert!(home
            .join(path_from_rel(HomeLayout::SETTINGS_FILE))
            .is_file());
        // First launch did apply migrations.
        assert!(!migrations_first.is_empty());

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn requested_factory_reset_recreates_a_fresh_home() {
        let home = temp_home();
        let first = bootstrap_home(&home).expect("first");
        let store = SettingsStore::new(&home);
        store
            .save_settings(&AppSettings {
                onboarding_completed: true,
                ..AppSettings::default()
            })
            .expect("save completed onboarding");
        let sentinel = home.join(HomeLayout::LOGS_DIR).join("sentinel.log");
        fs::write(&sentinel, b"private").expect("write sentinel");
        crate::permissions::harden_read_only_file(&sentinel).expect("make sentinel read-only");
        request_factory_reset(&home).expect("request reset");
        drop(first);

        let reset = bootstrap_home(&home).expect("reset bootstrap");
        assert!(reset.status.created_fresh);
        assert_eq!(reset.settings, AppSettings::default());
        assert!(!reset.settings.onboarding_completed);
        assert!(!sentinel.exists());
        assert!(!home.join(FACTORY_RESET_MARKER).exists());

        drop(reset);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn factory_reset_rejects_a_filesystem_root() {
        let root = Path::new(std::path::MAIN_SEPARATOR_STR);
        assert!(matches!(
            request_factory_reset(root),
            Err(StorageError::UnsafeFactoryResetTarget)
        ));
    }

    #[test]
    fn existing_database_is_backed_up_before_pending_migrations() {
        let home = temp_home();
        let db_path = home.join(path_from_rel(HomeLayout::DATABASE_FILE));
        fs::create_dir_all(db_path.parent().expect("parent")).expect("mkdir");
        {
            let database = Database::open(&db_path).expect("open legacy database");
            let conn = database.conn().expect("lock");
            conn.execute_batch(
                "CREATE TABLE legacy_probe (
                     id INTEGER PRIMARY KEY,
                     value TEXT NOT NULL
                 );
                 INSERT INTO legacy_probe (id, value) VALUES (1, 'preserved');",
            )
            .expect("seed legacy data");
        }

        let result = bootstrap_home(&home).expect("bootstrap");
        let latest = MIGRATIONS.last().map_or(0, |migration| migration.version);
        let backup_marker = format!("openconkit-db-v0-to-v{latest}");
        let database_backup = result
            .status
            .backups_created
            .iter()
            .find(|path| path.contains(&backup_marker))
            .expect("database backup");
        let backup_path = home.join(path_from_rel(database_backup));
        let backup = rusqlite::Connection::open(&backup_path).expect("open backup");
        let value: String = backup
            .query_row("SELECT value FROM legacy_probe WHERE id = 1", [], |row| {
                row.get(0)
            })
            .expect("query backup");
        assert_eq!(value, "preserved");

        drop(backup);
        drop(result);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn interrupted_bootstrap_is_detected() {
        let home = temp_home();
        fs::create_dir_all(&home).expect("mkdir");
        let marker = home.join(path_from_rel(INTERRUPT_MARKER));
        fs::create_dir_all(marker.parent().expect("parent")).expect("mkdir");
        fs::write(&marker, b"in-progress\n").expect("marker");

        let result = bootstrap_home(&home).expect("bootstrap");
        assert!(result.status.recovered_from_interrupt);
        assert!(!marker.exists(), "marker cleared after recovery");

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn corrupt_settings_on_launch_are_recovered() {
        let home = temp_home();
        // First launch to create layout.
        bootstrap_home(&home).expect("first");
        // Corrupt settings.
        let settings_path = home.join(path_from_rel(HomeLayout::SETTINGS_FILE));
        fs::write(&settings_path, b"{{{not json").expect("corrupt");

        let result = bootstrap_home(&home).expect("recover");
        assert!(!result.status.config_warnings.is_empty());
        assert_eq!(result.settings, AppSettings::default());
        assert!(!result.status.backups_created.is_empty());
        assert!(result.status.backups_created[0].contains("settings-corrupt"));

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn bootstrap_reconciles_update_state_from_canonical_settings() {
        let home = temp_home();
        bootstrap_home(&home).expect("first");
        let store = SettingsStore::new(&home);
        let settings = AppSettings {
            update_channel: openconkit_application::UpdateChannel::Beta,
            ..AppSettings::default()
        };
        store.save_settings(&settings).expect("save settings");

        let result = bootstrap_home(&home).expect("reconcile");
        assert_eq!(result.update_channel.channel, settings.update_channel);
        assert!(result
            .status
            .config_warnings
            .iter()
            .any(|warning| warning.contains("reconciled")));
        let (persisted, warnings, _) = store.load_update_channel().expect("load state");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(persisted.channel, settings.update_channel);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn home_override_is_explicitly_rejected_for_release_mode() {
        let result = resolve_home_from(
            Some(std::ffi::OsString::from("D:\\custom")),
            Some(std::ffi::OsString::from("C:\\Users\\user")),
            None,
            false,
        );
        assert!(matches!(result, Err(StorageError::HomeOverrideNotAllowed)));
    }

    #[test]
    fn canonical_home_is_used_without_development_override() {
        let result = resolve_home_from(
            None,
            Some(std::ffi::OsString::from("C:\\Users\\user")),
            None,
            false,
        )
        .expect("canonical home");
        assert_eq!(result, PathBuf::from("C:\\Users\\user").join(".openconkit"));
    }
}
