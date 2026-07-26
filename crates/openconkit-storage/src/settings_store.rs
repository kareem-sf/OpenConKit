//! Settings and update-channel file persistence under app home.

use std::fs;
use std::path::{Path, PathBuf};

use jiff::Timestamp;
use openconkit_application::{
    AppSettings, ConfigError, HomeLayout, UpdateChannelState, SETTINGS_SCHEMA_VERSION,
};

use crate::atomic::atomic_write;
use crate::permissions::harden_file;
use crate::StorageError;

/// Load / save application settings and updater state under app home.
pub struct SettingsStore {
    home: PathBuf,
}

impl SettingsStore {
    /// Create a store rooted at `home`.
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }

    /// Absolute path of `config/settings.json`.
    pub fn settings_path(&self) -> PathBuf {
        self.home.join(path_from_rel(HomeLayout::SETTINGS_FILE))
    }

    /// Absolute path of `config/update-channel.json`.
    pub fn update_channel_path(&self) -> PathBuf {
        self.home
            .join(path_from_rel(HomeLayout::UPDATE_CHANNEL_FILE))
    }

    /// Absolute path of the backups directory.
    pub fn backups_dir(&self) -> PathBuf {
        self.home.join(path_from_rel(HomeLayout::BACKUPS_DIR))
    }

    /// Load settings with per-field fail-safe semantics.
    ///
    /// Missing file → defaults (no warning). Corrupt/unparseable file →
    /// defaults + warning, and the corrupt file is backed up under
    /// `backups/`. Returns `(settings, warnings, backups_created)`.
    pub fn load_settings(&self) -> Result<(AppSettings, Vec<String>, Vec<String>), StorageError> {
        let path = self.settings_path();
        if !path.exists() {
            return Ok((AppSettings::default(), Vec::new(), Vec::new()));
        }
        let raw = fs::read_to_string(&path).map_err(io_err)?;
        let upgrade_legacy_schema = has_legacy_schema(&raw);
        let (settings, warnings) = AppSettings::from_json_str(&raw);
        let mut backups = Vec::new();
        if !warnings.is_empty() && raw_is_wholly_corrupt(&warnings) {
            if let Some(rel) = self.backup_corrupt(&path, "settings")? {
                backups.push(rel);
            }
            self.save_settings(&settings)?;
        } else if upgrade_legacy_schema {
            self.save_settings(&settings)?;
        }
        Ok((settings, warnings, backups))
    }

    /// Persist settings atomically.
    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), StorageError> {
        let json = settings
            .to_json_string()
            .map_err(|err| StorageError::Config(err.to_string()))?;
        atomic_write(&self.settings_path(), json.as_bytes()).map_err(io_err)?;
        Ok(())
    }

    /// Load updater state with the same fail-safe semantics as settings.
    pub fn load_update_channel(
        &self,
    ) -> Result<(UpdateChannelState, Vec<String>, Vec<String>), StorageError> {
        let path = self.update_channel_path();
        if !path.exists() {
            return Ok((UpdateChannelState::default(), Vec::new(), Vec::new()));
        }
        let raw = fs::read_to_string(&path).map_err(io_err)?;
        let upgrade_legacy_schema = has_legacy_schema(&raw);
        let (state, warnings) = UpdateChannelState::from_json_str(&raw);
        let mut backups = Vec::new();
        if !warnings.is_empty() && raw_is_wholly_corrupt(&warnings) {
            if let Some(rel) = self.backup_corrupt(&path, "update-channel")? {
                backups.push(rel);
            }
            self.save_update_channel(&state)?;
        } else if upgrade_legacy_schema {
            self.save_update_channel(&state)?;
        }
        Ok((state, warnings, backups))
    }

    /// Persist updater state atomically.
    pub fn save_update_channel(&self, state: &UpdateChannelState) -> Result<(), StorageError> {
        let json = state
            .to_json_string()
            .map_err(|err| StorageError::Config(err.to_string()))?;
        atomic_write(&self.update_channel_path(), json.as_bytes()).map_err(io_err)?;
        Ok(())
    }

    /// Ensure default config files exist (write defaults if missing).
    pub fn ensure_defaults(&self) -> Result<(), StorageError> {
        if !self.settings_path().exists() {
            self.save_settings(&AppSettings::default())?;
        }
        if !self.update_channel_path().exists() {
            self.save_update_channel(&UpdateChannelState::default())?;
        }
        Ok(())
    }

    fn backup_corrupt(&self, path: &Path, kind: &str) -> Result<Option<String>, StorageError> {
        if !path.exists() {
            return Ok(None);
        }
        let backups = self.backups_dir();
        fs::create_dir_all(&backups).map_err(io_err)?;
        let stamp = Timestamp::now().strftime("%Y%m%dT%H%M%SZ").to_string();
        let name = format!("{kind}-corrupt-{stamp}.json");
        let dest = backups.join(&name);
        fs::copy(path, &dest).map_err(io_err)?;
        harden_file(&dest).map_err(io_err)?;
        Ok(Some(format!("{}/{}", HomeLayout::BACKUPS_DIR, name)))
    }
}

fn raw_is_wholly_corrupt(warnings: &[String]) -> bool {
    warnings.iter().any(|w| {
        w.contains("not valid JSON")
            || w.contains("not a JSON object")
            || w.contains("all defaults restored")
    })
}

fn has_legacy_schema(raw: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|value| {
            value
                .get("schema_version")
                .and_then(serde_json::Value::as_u64)
        })
        .is_some_and(|version| version < u64::from(SETTINGS_SCHEMA_VERSION))
}

fn path_from_rel(rel: &str) -> PathBuf {
    rel.split('/').collect()
}

fn io_err(err: std::io::Error) -> StorageError {
    StorageError::Io {
        message: err.to_string(),
    }
}

// Keep ConfigError reachable for callers that want to map it.
#[allow(dead_code)]
fn _config_err_bridge(err: ConfigError) -> StorageError {
    StorageError::Config(err.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use openconkit_application::{Language, Theme};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_home() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("openconkit-settings-{nanos}"));
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn missing_settings_yields_defaults() {
        let home = temp_home();
        let store = SettingsStore::new(&home);
        let (settings, warnings, backups) = store.load_settings().expect("load");
        assert_eq!(settings, AppSettings::default());
        assert!(warnings.is_empty());
        assert!(backups.is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn save_and_load_round_trip() {
        let home = temp_home();
        let store = SettingsStore::new(&home);
        store.ensure_defaults().expect("defaults");
        let settings = AppSettings {
            onboarding_completed: true,
            language: Language::Ar,
            theme: Theme::Dark,
            ..AppSettings::default()
        };
        store.save_settings(&settings).expect("save");
        let (loaded, warnings, _) = store.load_settings().expect("load");
        assert!(warnings.is_empty());
        assert_eq!(loaded.language, Language::Ar);
        assert_eq!(loaded.theme, Theme::Dark);
        assert!(loaded.onboarding_completed);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn corrupt_settings_backed_up_and_defaults_restored() {
        let home = temp_home();
        let store = SettingsStore::new(&home);
        let path = store.settings_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(&path, b"NOT JSON {{{").expect("write corrupt");
        let (settings, warnings, backups) = store.load_settings().expect("load");
        assert_eq!(settings, AppSettings::default());
        assert!(!warnings.is_empty());
        assert_eq!(backups.len(), 1);
        assert!(backups[0].starts_with("backups/settings-corrupt-"));
        let repaired = fs::read_to_string(&path).expect("read repaired settings");
        assert!(serde_json::from_str::<serde_json::Value>(&repaired).is_ok());

        let (settings, warnings, backups) = store.load_settings().expect("load repaired");
        assert_eq!(settings, AppSettings::default());
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(backups.is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn corrupt_update_channel_is_backed_up_and_repaired() {
        let home = temp_home();
        let store = SettingsStore::new(&home);
        let path = store.update_channel_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(&path, b"NOT JSON {{{").expect("write corrupt");

        let (state, warnings, backups) = store.load_update_channel().expect("load");
        assert_eq!(state, UpdateChannelState::default());
        assert!(!warnings.is_empty());
        assert_eq!(backups.len(), 1);

        let (state, warnings, backups) = store.load_update_channel().expect("load repaired");
        assert_eq!(state, UpdateChannelState::default());
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(backups.is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn per_field_fallback_does_not_backup() {
        let home = temp_home();
        let store = SettingsStore::new(&home);
        let path = store.settings_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        // Valid JSON object with one bad field — fail-safe, no full restore.
        fs::write(
            &path,
            br#"{ "schema_version": 1, "language": "not-a-language", "theme": "dark" }"#,
        )
        .expect("write");
        let (settings, warnings, backups) = store.load_settings().expect("load");
        assert_eq!(settings.theme, Theme::Dark);
        assert_eq!(settings.language, Language::System); // fell back
        assert!(!warnings.is_empty());
        assert!(backups.is_empty());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn legacy_settings_are_upgraded_without_losing_user_choices() {
        let home = temp_home();
        let store = SettingsStore::new(&home);
        let path = store.settings_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(
            &path,
            br#"{
                "schema_version": 1,
                "onboarding_completed": true,
                "language": "ar",
                "theme": "dark",
                "update_channel": "beta",
                "tolerances": {
                    "absolute_tolerance": "0.25",
                    "relative_tolerance": "0.05",
                    "decimal_precision": 4
                },
                "privacy": {
                    "ai_features_enabled": false,
                    "diagnostic_logging_enabled": true
                },
                "advanced": {
                    "use_system_codex": false,
                    "system_codex_binary": null
                },
                "last_successful_update_check": null
            }"#,
        )
        .expect("write legacy settings");

        let (settings, warnings, backups) = store.load_settings().expect("upgrade");
        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
        assert!(settings.onboarding_completed);
        assert_eq!(settings.language, Language::Ar);
        assert_eq!(settings.theme, Theme::Dark);
        assert!(
            warnings.iter().any(|warning| warning.contains("upgraded")),
            "{warnings:?}"
        );
        assert!(backups.is_empty());

        let persisted: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read upgraded settings"))
                .expect("valid upgraded settings");
        assert_eq!(
            persisted
                .get("schema_version")
                .and_then(serde_json::Value::as_u64),
            Some(u64::from(SETTINGS_SCHEMA_VERSION))
        );

        let (reloaded, warnings, _) = store.load_settings().expect("reload upgraded settings");
        assert_eq!(reloaded, settings);
        assert!(warnings.is_empty(), "{warnings:?}");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn legacy_update_channel_is_upgraded() {
        let home = temp_home();
        let store = SettingsStore::new(&home);
        let path = store.update_channel_path();
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(
            &path,
            br#"{
                "schema_version": 1,
                "channel": "beta",
                "last_successful_update_check": null
            }"#,
        )
        .expect("write legacy updater state");

        let (state, warnings, backups) = store.load_update_channel().expect("upgrade");
        assert_eq!(state.schema_version, SETTINGS_SCHEMA_VERSION);
        assert!(
            warnings.iter().any(|warning| warning.contains("upgraded")),
            "{warnings:?}"
        );
        assert!(backups.is_empty());

        let persisted: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read upgraded updater state"))
                .expect("valid upgraded updater state");
        assert_eq!(
            persisted
                .get("schema_version")
                .and_then(serde_json::Value::as_u64),
            Some(u64::from(SETTINGS_SCHEMA_VERSION))
        );
        let _ = fs::remove_dir_all(&home);
    }
}
