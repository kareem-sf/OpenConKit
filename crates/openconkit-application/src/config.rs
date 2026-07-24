//! Schema-versioned application settings with per-field fail-safe loading.
//!
//! Settings files are user-writable and can be corrupted or hand-edited, so
//! deserialization NEVER fails wholesale: each field is extracted
//! individually and falls back to its default with a human-readable warning.
//! Unknown fields are ignored for forward compatibility.

use jiff::Timestamp;
use rust_decimal::Decimal;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::Path;
use ts_rs::TS;

use openconkit_domain::ErrorCode;

/// Current schema version of `settings.json`.
pub const SETTINGS_SCHEMA_VERSION: u32 = 2;

/// UI language. `System` follows the OS locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum Language {
    /// Follow the operating system locale.
    System,
    /// English.
    En,
    /// Arabic.
    Ar,
}

/// Color theme. `System` follows the OS preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum Theme {
    /// Follow the operating system preference.
    System,
    /// Light theme.
    Light,
    /// Dark theme.
    Dark,
}

/// Update feed channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum UpdateChannel {
    /// Stable releases only.
    Stable,
    /// Beta/pre-release channel.
    Beta,
}

/// Numeric tolerances used by analysis rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AnalysisTolerances {
    /// Absolute tolerance for quantity/amount comparisons (JSON string).
    #[ts(type = "string")]
    pub absolute_tolerance: Decimal,
    /// Relative tolerance for quantity/amount comparisons (JSON string).
    #[ts(type = "string")]
    pub relative_tolerance: Decimal,
    /// Decimal places values are rounded to before comparison (0..=6).
    pub decimal_precision: u8,
}

impl AnalysisTolerances {
    /// Maximum supported decimal precision.
    pub const MAX_DECIMAL_PRECISION: u8 = 6;

    /// Clamp out-of-range values into the supported range.
    pub fn sanitize(&mut self) {
        self.decimal_precision = self.decimal_precision.min(Self::MAX_DECIMAL_PRECISION);
    }

    fn validate(&self) -> Result<(), ConfigError> {
        for (field, value) in [
            ("tolerances.absolute_tolerance", self.absolute_tolerance),
            ("tolerances.relative_tolerance", self.relative_tolerance),
        ] {
            if value < Decimal::ZERO {
                return Err(ConfigError::InvalidPatch {
                    field: field.to_string(),
                    message: "tolerance must be non-negative".to_string(),
                });
            }
        }
        Ok(())
    }
}

impl Default for AnalysisTolerances {
    fn default() -> Self {
        Self {
            absolute_tolerance: Decimal::new(1, 2),
            relative_tolerance: Decimal::new(1, 3),
            decimal_precision: 2,
        }
    }
}

/// Privacy-related switches. Everything defaults to OFF: the app is
/// local-first with no telemetry, and AI features are opt-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
pub struct PrivacySettings {
    /// Whether the user has explicitly enabled AI features (default false).
    pub ai_features_enabled: bool,
    /// Whether local diagnostic logging is enabled (default false).
    pub diagnostic_logging_enabled: bool,
}

/// Development/debugging overrides hidden under Advanced settings.
///
/// Production defaults always use the bundled, pinned Codex app-server.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
pub struct AdvancedSettings {
    /// Use an explicitly selected system `codex` CLI after a restart.
    pub use_system_codex: bool,
    /// Absolute path to the selected CLI. It is never interpreted by a shell.
    pub system_codex_binary: Option<String>,
}

impl AdvancedSettings {
    fn validate(&self) -> Result<(), ConfigError> {
        let Some(raw_path) = self.system_codex_binary.as_deref() else {
            if self.use_system_codex {
                return Err(ConfigError::InvalidPatch {
                    field: "advanced.system_codex_binary".to_string(),
                    message: "a system Codex path is required when the override is enabled"
                        .to_string(),
                });
            }
            return Ok(());
        };
        let path = Path::new(raw_path);
        let valid_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "codex" || name.eq_ignore_ascii_case("codex.exe"));
        if raw_path.trim() != raw_path
            || raw_path.len() > 1_024
            || raw_path.contains('\0')
            || !path.is_absolute()
            || !valid_name
        {
            return Err(ConfigError::InvalidPatch {
                field: "advanced.system_codex_binary".to_string(),
                message: "system Codex must be an absolute path ending in codex or codex.exe"
                    .to_string(),
            });
        }
        Ok(())
    }
}

/// Errors from settings (de)serialization and patching.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The whole settings file could not be parsed as JSON. Callers should
    /// regenerate defaults (back up the corrupt file first).
    #[error("malformed JSON: {message}")]
    MalformedJson {
        /// Parser error detail.
        message: String,
    },

    /// A patch contained an unknown field or an invalid value.
    #[error("invalid patch for field {field:?}: {message}")]
    InvalidPatch {
        /// The offending field name.
        field: String,
        /// Why the value was rejected.
        message: String,
    },

    /// Serializing settings to JSON failed.
    #[error("failed to serialize settings: {message}")]
    Serialize {
        /// Serializer error detail.
        message: String,
    },
}

impl ErrorCode for ConfigError {
    fn code(&self) -> &'static str {
        match self {
            ConfigError::MalformedJson { .. } => "CONFIG_MALFORMED_JSON",
            ConfigError::InvalidPatch { .. } => "CONFIG_INVALID_PATCH",
            ConfigError::Serialize { .. } => "CONFIG_SERIALIZE",
        }
    }
}

/// Extract `field` from `obj` as `T`, falling back to `fallback` with a
/// warning when the value is present but invalid. Missing fields fall back
/// silently unless `warn_on_missing` is set.
fn field_or_default<T: DeserializeOwned>(
    obj: &Map<String, Value>,
    field: &str,
    fallback: T,
    warn_on_missing: bool,
    warnings: &mut Vec<String>,
) -> T {
    match obj.get(field) {
        None => {
            if warn_on_missing {
                warnings.push(format!("field {field:?} is missing; using default"));
            }
            fallback
        }
        Some(value) => match serde_json::from_value::<T>(value.clone()) {
            Ok(parsed) => parsed,
            Err(err) => {
                warnings.push(format!(
                    "field {field:?} has an invalid value ({err}); using default"
                ));
                fallback
            }
        },
    }
}

/// Root application settings stored in `config/settings.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct AppSettings {
    /// Schema version of the settings file.
    pub schema_version: u32,
    /// Whether the user completed the local-first privacy welcome.
    pub onboarding_completed: bool,
    /// UI language.
    pub language: Language,
    /// Color theme.
    pub theme: Theme,
    /// Update feed channel.
    pub update_channel: UpdateChannel,
    /// Analysis tolerances.
    pub tolerances: AnalysisTolerances,
    /// Privacy switches.
    pub privacy: PrivacySettings,
    /// Development/debugging overrides.
    pub advanced: AdvancedSettings,
    /// When the updater last successfully checked for updates.
    pub last_successful_update_check: Option<Timestamp>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            onboarding_completed: false,
            language: Language::System,
            theme: Theme::System,
            update_channel: UpdateChannel::Stable,
            tolerances: AnalysisTolerances::default(),
            privacy: PrivacySettings::default(),
            advanced: AdvancedSettings::default(),
            last_successful_update_check: None,
        }
    }
}

impl AppSettings {
    /// Load settings from a JSON string, degrading per field.
    ///
    /// This never fails: a wholly unparseable file yields defaults plus a
    /// warning, and individually invalid fields fall back to their defaults
    /// with warnings naming the field. Unknown fields are ignored.
    pub fn from_json_str(raw: &str) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let value: Value = match serde_json::from_str(raw) {
            Ok(value) => value,
            Err(err) => {
                warnings.push(format!(
                    "settings file is not valid JSON ({err}); all defaults restored"
                ));
                return (Self::default(), warnings);
            }
        };
        let Some(obj) = value.as_object() else {
            warnings.push("settings file is not a JSON object; all defaults restored".to_string());
            return (Self::default(), warnings);
        };

        let default = Self::default();
        let schema_version = field_or_default(
            obj,
            "schema_version",
            default.schema_version,
            true,
            &mut warnings,
        );
        let onboarding_completed = field_or_default(
            obj,
            "onboarding_completed",
            default.onboarding_completed,
            false,
            &mut warnings,
        );
        let language = field_or_default(obj, "language", default.language, false, &mut warnings);
        let theme = field_or_default(obj, "theme", default.theme, false, &mut warnings);
        let update_channel = field_or_default(
            obj,
            "update_channel",
            default.update_channel,
            false,
            &mut warnings,
        );
        let tolerances = tolerances_from_json(obj.get("tolerances"), &mut warnings);
        let privacy = privacy_from_json(obj.get("privacy"), &mut warnings);
        let advanced = advanced_from_json(obj.get("advanced"), &mut warnings);
        let last_successful_update_check = field_or_default(
            obj,
            "last_successful_update_check",
            default.last_successful_update_check,
            false,
            &mut warnings,
        );

        (
            Self {
                schema_version,
                onboarding_completed,
                language,
                theme,
                update_channel,
                tolerances,
                privacy,
                advanced,
                last_successful_update_check,
            },
            warnings,
        )
    }

    /// Serialize to pretty-printed JSON.
    pub fn to_json_string(&self) -> Result<String, ConfigError> {
        serde_json::to_string_pretty(self).map_err(|err| ConfigError::Serialize {
            message: err.to_string(),
        })
    }

    /// Apply a JSON patch object. Only known fields are patched, each with
    /// per-field validation. Returns the names of the applied fields
    /// (nested fields use dotted names). An unknown or invalid field aborts
    /// the patch with [`ConfigError::InvalidPatch`]. An empty patch is a
    /// no-op.
    pub fn apply_patch(&mut self, patch: &Value) -> Result<Vec<String>, ConfigError> {
        let mut next = self.clone();
        let applied = next.apply_patch_in_place(patch)?;
        *self = next;
        Ok(applied)
    }

    fn apply_patch_in_place(&mut self, patch: &Value) -> Result<Vec<String>, ConfigError> {
        let obj = patch.as_object().ok_or_else(|| ConfigError::InvalidPatch {
            field: "<root>".to_string(),
            message: "patch must be a JSON object".to_string(),
        })?;
        let mut applied = Vec::new();
        for (key, value) in obj {
            match key.as_str() {
                "language" => {
                    self.language = parse_patch_field(key, value)?;
                    applied.push(key.clone());
                }
                "onboarding_completed" => {
                    self.onboarding_completed = parse_patch_field(key, value)?;
                    applied.push(key.clone());
                }
                "theme" => {
                    self.theme = parse_patch_field(key, value)?;
                    applied.push(key.clone());
                }
                "update_channel" => {
                    self.update_channel = parse_patch_field(key, value)?;
                    applied.push(key.clone());
                }
                "last_successful_update_check" => {
                    self.last_successful_update_check = parse_patch_field(key, value)?;
                    applied.push(key.clone());
                }
                "tolerances" => {
                    let nested = patch_object(key, value)?;
                    for (sub, sub_value) in nested {
                        let name = format!("tolerances.{sub}");
                        match sub.as_str() {
                            "absolute_tolerance" => {
                                self.tolerances.absolute_tolerance =
                                    parse_patch_field(&name, sub_value)?;
                            }
                            "relative_tolerance" => {
                                self.tolerances.relative_tolerance =
                                    parse_patch_field(&name, sub_value)?;
                            }
                            "decimal_precision" => {
                                let precision: u8 = parse_patch_field(&name, sub_value)?;
                                if precision > AnalysisTolerances::MAX_DECIMAL_PRECISION {
                                    return Err(ConfigError::InvalidPatch {
                                        field: name,
                                        message: format!(
                                            "decimal precision must be 0..={}",
                                            AnalysisTolerances::MAX_DECIMAL_PRECISION
                                        ),
                                    });
                                }
                                self.tolerances.decimal_precision = precision;
                            }
                            _ => {
                                return Err(ConfigError::InvalidPatch {
                                    field: name,
                                    message: "unknown settings field".to_string(),
                                });
                            }
                        }
                        applied.push(name);
                    }
                }
                "privacy" => {
                    let nested = patch_object(key, value)?;
                    for (sub, sub_value) in nested {
                        let name = format!("privacy.{sub}");
                        match sub.as_str() {
                            "ai_features_enabled" => {
                                self.privacy.ai_features_enabled =
                                    parse_patch_field(&name, sub_value)?;
                            }
                            "diagnostic_logging_enabled" => {
                                self.privacy.diagnostic_logging_enabled =
                                    parse_patch_field(&name, sub_value)?;
                            }
                            _ => {
                                return Err(ConfigError::InvalidPatch {
                                    field: name,
                                    message: "unknown settings field".to_string(),
                                });
                            }
                        }
                        applied.push(name);
                    }
                }
                "advanced" => {
                    let nested = patch_object(key, value)?;
                    for (sub, sub_value) in nested {
                        let name = format!("advanced.{sub}");
                        match sub.as_str() {
                            "use_system_codex" => {
                                self.advanced.use_system_codex =
                                    parse_patch_field(&name, sub_value)?;
                            }
                            "system_codex_binary" => {
                                self.advanced.system_codex_binary =
                                    parse_patch_field(&name, sub_value)?;
                            }
                            _ => {
                                return Err(ConfigError::InvalidPatch {
                                    field: name,
                                    message: "unknown settings field".to_string(),
                                });
                            }
                        }
                        applied.push(name);
                    }
                }
                _ => {
                    return Err(ConfigError::InvalidPatch {
                        field: key.clone(),
                        message: "unknown settings field".to_string(),
                    });
                }
            }
        }
        self.tolerances.validate()?;
        self.advanced.validate()?;
        Ok(applied)
    }

    /// Apply a typed patch (used by the IPC layer). `None` fields are left
    /// untouched; the tolerances are sanitized after patching.
    pub fn apply_typed_patch(&mut self, patch: SettingsPatch) -> Result<(), ConfigError> {
        let mut next = self.clone();
        if let Some(onboarding_completed) = patch.onboarding_completed {
            next.onboarding_completed = onboarding_completed;
        }
        if let Some(language) = patch.language {
            next.language = language;
        }
        if let Some(theme) = patch.theme {
            next.theme = theme;
        }
        if let Some(update_channel) = patch.update_channel {
            next.update_channel = update_channel;
        }
        if let Some(mut tolerances) = patch.tolerances {
            tolerances.sanitize();
            tolerances.validate()?;
            next.tolerances = tolerances;
        }
        if let Some(privacy) = patch.privacy {
            next.privacy = privacy;
        }
        if let Some(advanced) = patch.advanced {
            advanced.validate()?;
            next.advanced = advanced;
        }
        if let Some(last_successful_update_check) = patch.last_successful_update_check {
            next.last_successful_update_check = Some(last_successful_update_check);
        }
        *self = next;
        Ok(())
    }
}

/// Typed settings patch for the IPC layer: every field is optional and only
/// `Some` fields are applied.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct SettingsPatch {
    /// Mark the local-first privacy welcome as completed.
    pub onboarding_completed: Option<bool>,
    /// New UI language, if changing.
    pub language: Option<Language>,
    /// New theme, if changing.
    pub theme: Option<Theme>,
    /// New update channel, if changing.
    pub update_channel: Option<UpdateChannel>,
    /// Replacement tolerances (sanitized on apply), if changing.
    pub tolerances: Option<AnalysisTolerances>,
    /// Replacement privacy settings, if changing.
    pub privacy: Option<PrivacySettings>,
    /// Replacement development/debugging settings, if changing.
    pub advanced: Option<AdvancedSettings>,
    /// Set the last successful update-check timestamp.
    pub last_successful_update_check: Option<Timestamp>,
}

fn patch_object<'a>(field: &str, value: &'a Value) -> Result<&'a Map<String, Value>, ConfigError> {
    value.as_object().ok_or_else(|| ConfigError::InvalidPatch {
        field: field.to_string(),
        message: "expected a JSON object".to_string(),
    })
}

fn parse_patch_field<T: DeserializeOwned>(field: &str, value: &Value) -> Result<T, ConfigError> {
    serde_json::from_value(value.clone()).map_err(|err| ConfigError::InvalidPatch {
        field: field.to_string(),
        message: err.to_string(),
    })
}

fn tolerances_from_json(value: Option<&Value>, warnings: &mut Vec<String>) -> AnalysisTolerances {
    let default = AnalysisTolerances::default();
    let Some(value) = value else {
        return default;
    };
    let Some(obj) = value.as_object() else {
        warnings.push("field \"tolerances\" is not an object; using defaults".to_string());
        return default;
    };
    let mut tolerances = AnalysisTolerances {
        absolute_tolerance: field_or_default(
            obj,
            "absolute_tolerance",
            default.absolute_tolerance,
            false,
            warnings,
        ),
        relative_tolerance: field_or_default(
            obj,
            "relative_tolerance",
            default.relative_tolerance,
            false,
            warnings,
        ),
        decimal_precision: field_or_default(
            obj,
            "decimal_precision",
            default.decimal_precision,
            false,
            warnings,
        ),
    };
    if tolerances.decimal_precision > AnalysisTolerances::MAX_DECIMAL_PRECISION {
        warnings.push(format!(
            "field \"decimal_precision\" is out of range 0..={}; using default",
            AnalysisTolerances::MAX_DECIMAL_PRECISION
        ));
        tolerances.decimal_precision = default.decimal_precision;
    }
    if tolerances.absolute_tolerance < Decimal::ZERO {
        warnings
            .push("field \"absolute_tolerance\" must be non-negative; using default".to_string());
        tolerances.absolute_tolerance = default.absolute_tolerance;
    }
    if tolerances.relative_tolerance < Decimal::ZERO {
        warnings
            .push("field \"relative_tolerance\" must be non-negative; using default".to_string());
        tolerances.relative_tolerance = default.relative_tolerance;
    }
    tolerances
}

fn privacy_from_json(value: Option<&Value>, warnings: &mut Vec<String>) -> PrivacySettings {
    let default = PrivacySettings::default();
    let Some(value) = value else {
        return default;
    };
    let Some(obj) = value.as_object() else {
        warnings.push("field \"privacy\" is not an object; using defaults".to_string());
        return default;
    };
    PrivacySettings {
        ai_features_enabled: field_or_default(
            obj,
            "ai_features_enabled",
            default.ai_features_enabled,
            false,
            warnings,
        ),
        diagnostic_logging_enabled: field_or_default(
            obj,
            "diagnostic_logging_enabled",
            default.diagnostic_logging_enabled,
            false,
            warnings,
        ),
    }
}

fn advanced_from_json(value: Option<&Value>, warnings: &mut Vec<String>) -> AdvancedSettings {
    let default = AdvancedSettings::default();
    let Some(value) = value else {
        return default;
    };
    let Some(obj) = value.as_object() else {
        warnings.push("field \"advanced\" is not an object; using defaults".to_string());
        return default;
    };
    let advanced = AdvancedSettings {
        use_system_codex: field_or_default(
            obj,
            "use_system_codex",
            default.use_system_codex,
            false,
            warnings,
        ),
        system_codex_binary: field_or_default(
            obj,
            "system_codex_binary",
            default.system_codex_binary,
            false,
            warnings,
        ),
    };
    if let Err(error) = advanced.validate() {
        warnings.push(format!(
            "field \"advanced\" is invalid ({error}); using defaults"
        ));
        return AdvancedSettings::default();
    }
    advanced
}

/// Updater state stored in `config/update-channel.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct UpdateChannelState {
    /// Schema version of the state file.
    pub schema_version: u32,
    /// Selected update channel.
    pub channel: UpdateChannel,
    /// When the updater last successfully checked for updates.
    pub last_successful_update_check: Option<Timestamp>,
}

impl Default for UpdateChannelState {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            channel: UpdateChannel::Stable,
            last_successful_update_check: None,
        }
    }
}

impl UpdateChannelState {
    /// Load updater state from a JSON string with the same per-field
    /// fail-safe semantics as [`AppSettings::from_json_str`].
    pub fn from_json_str(raw: &str) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let value: Value = match serde_json::from_str(raw) {
            Ok(value) => value,
            Err(err) => {
                warnings.push(format!(
                    "update-channel file is not valid JSON ({err}); all defaults restored"
                ));
                return (Self::default(), warnings);
            }
        };
        let Some(obj) = value.as_object() else {
            warnings.push(
                "update-channel file is not a JSON object; all defaults restored".to_string(),
            );
            return (Self::default(), warnings);
        };

        let default = Self::default();
        (
            Self {
                schema_version: field_or_default(
                    obj,
                    "schema_version",
                    default.schema_version,
                    true,
                    &mut warnings,
                ),
                channel: field_or_default(obj, "channel", default.channel, false, &mut warnings),
                last_successful_update_check: field_or_default(
                    obj,
                    "last_successful_update_check",
                    default.last_successful_update_check,
                    false,
                    &mut warnings,
                ),
            },
            warnings,
        )
    }

    /// Serialize to pretty-printed JSON.
    pub fn to_json_string(&self) -> Result<String, ConfigError> {
        serde_json::to_string_pretty(self).map_err(|err| ConfigError::Serialize {
            message: err.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn defaults_are_valid_and_private() {
        let settings = AppSettings::default();
        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
        assert!(!settings.onboarding_completed);
        assert_eq!(settings.language, Language::System);
        assert_eq!(settings.theme, Theme::System);
        assert_eq!(settings.update_channel, UpdateChannel::Stable);
        assert_eq!(settings.tolerances.absolute_tolerance, Decimal::new(1, 2));
        assert_eq!(settings.tolerances.relative_tolerance, Decimal::new(1, 3));
        assert_eq!(settings.tolerances.decimal_precision, 2);
        assert!(!settings.privacy.ai_features_enabled);
        assert!(!settings.privacy.diagnostic_logging_enabled);
        assert!(!settings.advanced.use_system_codex);
        assert!(settings.advanced.system_codex_binary.is_none());
        assert!(settings.last_successful_update_check.is_none());
    }

    #[test]
    fn serde_round_trip_preserves_everything() {
        let mut settings = AppSettings {
            language: Language::Ar,
            theme: Theme::Dark,
            tolerances: AnalysisTolerances {
                decimal_precision: 4,
                ..AnalysisTolerances::default()
            },
            ..AppSettings::default()
        };
        settings.privacy.ai_features_enabled = true;
        let json = settings.to_json_string().expect("serialize");
        let (back, warnings) = AppSettings::from_json_str(&json);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(back, settings);
        // Decimals serialize as strings (serde-with-str).
        assert!(json.contains("\"absolute_tolerance\": \"0.01\""), "{json}");
    }

    #[test]
    fn invalid_fields_fall_back_individually() {
        let raw = r#"{
            "schema_version": 1,
            "language": "ar",
            "theme": "neon",
            "tolerances": { "absolute_tolerance": "0.5", "decimal_precision": 99 }
        }"#;
        let (settings, warnings) = AppSettings::from_json_str(raw);
        // Valid fields preserved.
        assert_eq!(settings.language, Language::Ar);
        assert!(!settings.onboarding_completed);
        assert_eq!(settings.tolerances.absolute_tolerance, Decimal::new(5, 1));
        // Invalid fields defaulted, with warnings naming them.
        assert_eq!(settings.theme, Theme::System);
        assert_eq!(settings.tolerances.decimal_precision, 2);
        assert!(warnings.iter().any(|w| w.contains("theme")), "{warnings:?}");
        assert!(
            warnings.iter().any(|w| w.contains("decimal_precision")),
            "{warnings:?}"
        );
    }

    #[test]
    fn onboarding_completion_is_explicit_and_patchable() {
        let (mut settings, warnings) = AppSettings::from_json_str(r#"{"schema_version": 1}"#);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(!settings.onboarding_completed);

        settings
            .apply_patch(&serde_json::json!({"onboarding_completed": true}))
            .expect("complete onboarding");
        assert!(settings.onboarding_completed);
    }

    #[test]
    fn garbage_file_yields_defaults_and_warning() {
        let (settings, warnings) = AppSettings::from_json_str("{ not json !!!");
        assert_eq!(settings, AppSettings::default());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("not valid JSON"));
    }

    #[test]
    fn non_object_file_yields_defaults_and_warning() {
        let (settings, warnings) = AppSettings::from_json_str("[1, 2, 3]");
        assert_eq!(settings, AppSettings::default());
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let raw = r#"{"schema_version": 1, "future_field": {"x": 1}, "theme": "dark"}"#;
        let (settings, warnings) = AppSettings::from_json_str(raw);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(settings.theme, Theme::Dark);
    }

    #[test]
    fn missing_schema_version_warns() {
        let (settings, warnings) = AppSettings::from_json_str(r#"{"theme": "light"}"#);
        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
        assert!(
            warnings.iter().any(|w| w.contains("schema_version")),
            "{warnings:?}"
        );
    }

    #[test]
    fn apply_patch_updates_known_fields() {
        let mut settings = AppSettings::default();
        let patch = serde_json::json!({
            "onboarding_completed": true,
            "theme": "dark",
            "privacy": { "ai_features_enabled": true },
            "tolerances": { "decimal_precision": 3 }
        });
        let applied = settings.apply_patch(&patch).expect("patch applies");
        assert!(settings.onboarding_completed);
        assert_eq!(settings.theme, Theme::Dark);
        assert!(settings.privacy.ai_features_enabled);
        assert!(!settings.privacy.diagnostic_logging_enabled);
        assert_eq!(settings.tolerances.decimal_precision, 3);
        let mut applied = applied;
        applied.sort();
        assert_eq!(
            applied,
            vec![
                "onboarding_completed".to_string(),
                "privacy.ai_features_enabled".to_string(),
                "theme".to_string(),
                "tolerances.decimal_precision".to_string()
            ]
        );
    }

    #[test]
    fn apply_patch_rejects_unknown_and_invalid_fields() {
        let mut settings = AppSettings::default();
        let err = settings
            .apply_patch(&serde_json::json!({"nope": 1}))
            .expect_err("unknown field rejected");
        assert!(matches!(
            err,
            ConfigError::InvalidPatch { ref field, .. } if field == "nope"
        ));

        let err = settings
            .apply_patch(&serde_json::json!({"theme": "neon"}))
            .expect_err("invalid value rejected");
        assert!(matches!(
            err,
            ConfigError::InvalidPatch { ref field, .. } if field == "theme"
        ));

        let err = settings
            .apply_patch(&serde_json::json!({"tolerances": {"decimal_precision": 99}}))
            .expect_err("out-of-range precision rejected");
        assert!(matches!(err, ConfigError::InvalidPatch { .. }));

        let err = settings
            .apply_patch(&serde_json::json!(["not", "an", "object"]))
            .expect_err("non-object rejected");
        assert!(matches!(err, ConfigError::InvalidPatch { .. }));
    }

    #[test]
    fn rejected_patch_does_not_partially_mutate_settings() {
        let mut settings = AppSettings::default();
        let original = settings.clone();
        let err = settings
            .apply_patch(&serde_json::json!({
                "theme": "dark",
                "privacy": { "unknown": true }
            }))
            .expect_err("whole patch must be rejected");
        assert!(matches!(err, ConfigError::InvalidPatch { .. }));
        assert_eq!(settings, original);
    }

    #[test]
    fn apply_patch_accepts_empty_object() {
        let mut settings = AppSettings::default();
        let applied = settings
            .apply_patch(&serde_json::json!({}))
            .expect("empty patch ok");
        assert!(applied.is_empty());
    }

    #[test]
    fn typed_patch_applies_some_fields_and_sanitizes() {
        let mut settings = AppSettings::default();
        let patch = SettingsPatch {
            onboarding_completed: Some(true),
            theme: Some(Theme::Light),
            tolerances: Some(AnalysisTolerances {
                absolute_tolerance: Decimal::new(1, 1),
                relative_tolerance: Decimal::new(1, 2),
                decimal_precision: 42,
            }),
            ..SettingsPatch::default()
        };
        settings
            .apply_typed_patch(patch)
            .expect("typed patch applies");
        assert_eq!(settings.theme, Theme::Light);
        assert!(settings.onboarding_completed);
        assert_eq!(settings.language, Language::System);
        assert_eq!(settings.tolerances.absolute_tolerance, Decimal::new(1, 1));
        assert_eq!(settings.tolerances.decimal_precision, 6);
    }

    #[test]
    fn typed_patch_rejects_negative_tolerances_without_mutation() {
        let mut settings = AppSettings::default();
        let original = settings.clone();
        let patch = SettingsPatch {
            theme: Some(Theme::Dark),
            tolerances: Some(AnalysisTolerances {
                absolute_tolerance: Decimal::new(-1, 0),
                ..AnalysisTolerances::default()
            }),
            ..SettingsPatch::default()
        };
        let err = settings
            .apply_typed_patch(patch)
            .expect_err("negative tolerance rejected");
        assert!(matches!(err, ConfigError::InvalidPatch { .. }));
        assert_eq!(settings, original);
    }

    #[test]
    fn advanced_system_codex_requires_an_absolute_codex_executable() {
        let mut settings = AppSettings::default();
        let executable_name = if cfg!(windows) { "codex.exe" } else { "codex" };
        let valid_path = std::env::temp_dir()
            .join(executable_name)
            .to_string_lossy()
            .into_owned();
        settings
            .apply_typed_patch(SettingsPatch {
                advanced: Some(AdvancedSettings {
                    use_system_codex: true,
                    system_codex_binary: Some(valid_path.clone()),
                }),
                ..SettingsPatch::default()
            })
            .expect("valid system CLI override");
        assert!(settings.advanced.use_system_codex);
        assert_eq!(
            settings.advanced.system_codex_binary.as_deref(),
            Some(valid_path.as_str())
        );

        let original = settings.clone();
        let error = settings
            .apply_typed_patch(SettingsPatch {
                advanced: Some(AdvancedSettings {
                    use_system_codex: true,
                    system_codex_binary: Some("relative/codex".to_string()),
                }),
                ..SettingsPatch::default()
            })
            .expect_err("relative executable rejected");
        assert!(matches!(error, ConfigError::InvalidPatch { .. }));
        assert_eq!(settings, original);
    }

    #[test]
    fn sanitize_clamps_precision() {
        let mut tolerances = AnalysisTolerances {
            decimal_precision: 250,
            ..AnalysisTolerances::default()
        };
        tolerances.sanitize();
        assert_eq!(
            tolerances.decimal_precision,
            AnalysisTolerances::MAX_DECIMAL_PRECISION
        );
    }

    #[test]
    fn update_channel_state_round_trip_and_fail_safe() {
        let state = UpdateChannelState::default();
        let json = state.to_json_string().expect("serialize");
        let (back, warnings) = UpdateChannelState::from_json_str(&json);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(back, state);

        let (state, warnings) =
            UpdateChannelState::from_json_str(r#"{"channel": "nightly", "feed_url": "https://x"}"#);
        assert_eq!(state.channel, UpdateChannel::Stable);
        assert!(warnings.iter().any(|w| w.contains("channel")));
        assert!(warnings.iter().any(|w| w.contains("schema_version")));

        let (state, warnings) = UpdateChannelState::from_json_str("%%%");
        assert_eq!(state, UpdateChannelState::default());
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn config_error_codes_are_stable() {
        let cases: [(ConfigError, &str); 3] = [
            (
                ConfigError::MalformedJson {
                    message: "x".into(),
                },
                "CONFIG_MALFORMED_JSON",
            ),
            (
                ConfigError::InvalidPatch {
                    field: "x".into(),
                    message: "y".into(),
                },
                "CONFIG_INVALID_PATCH",
            ),
            (
                ConfigError::Serialize {
                    message: "x".into(),
                },
                "CONFIG_SERIALIZE",
            ),
        ];
        for (err, expected) in cases {
            assert_eq!(err.code(), expected);
        }
    }
}
