//! Isolated, privacy-preserving Codex profile preparation.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::CodexError;

const CONFIG: &str = r#"forced_login_method = "chatgpt"
cli_auth_credentials_store = "auto"
check_for_update_on_startup = false
web_search = "disabled"
approval_policy = "never"
sandbox_mode = "read-only"
allow_login_shell = false

[analytics]
enabled = false

[feedback]
enabled = false

[features]
apps = false
goals = false
hooks = false
multi_agent = false
remote_plugin = false
shell_snapshot = false
shell_tool = false
"#;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

/// Create and harden OpenConKit's isolated Codex home and controlled config.
///
/// The profile never inherits a user's ordinary Codex configuration. Codex
/// itself owns authentication material and prefers the OS credential store.
pub fn prepare_codex_home(path: &Path) -> Result<PathBuf, CodexError> {
    if !path.is_absolute() {
        return Err(CodexError::InvalidConfiguration(
            "CODEX_HOME must be absolute".to_string(),
        ));
    }
    fs::create_dir_all(path)?;
    harden_directory(path)?;

    let config_path = path.join("config.toml");
    let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let temporary_path = path.join(format!(
        ".config.toml.{}-{sequence}.tmp",
        std::process::id()
    ));
    let result = (|| {
        let mut temporary = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)?;
        harden_file(&temporary_path)?;
        temporary.write_all(CONFIG.as_bytes())?;
        temporary.sync_all()?;
        drop(temporary);
        fs::rename(&temporary_path, &config_path)?;
        harden_file(&config_path)?;
        sync_directory(path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result?;
    Ok(config_path)
}

fn harden_directory(path: &Path) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn harden_file(path: &Path) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        fs::File::open(path)?.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temporary_home() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("openconkit-codex-profile-{suffix}"))
    }

    #[test]
    fn writes_fail_closed_local_first_config() {
        let home = temporary_home();
        let config = prepare_codex_home(&home).expect("profile");
        let contents = fs::read_to_string(config).expect("config");
        for required in [
            "forced_login_method = \"chatgpt\"",
            "cli_auth_credentials_store = \"auto\"",
            "check_for_update_on_startup = false",
            "web_search = \"disabled\"",
            "enabled = false",
            "shell_tool = false",
            "multi_agent = false",
        ] {
            assert!(contents.contains(required), "missing {required}");
        }
        fs::remove_dir_all(home).expect("cleanup");
    }

    #[test]
    fn replacing_config_removes_uncontrolled_values() {
        let home = temporary_home();
        fs::create_dir_all(&home).expect("mkdir");
        fs::write(home.join("config.toml"), "model_provider = \"custom\"\n").expect("seed");
        let config = prepare_codex_home(&home).expect("profile");
        let contents = fs::read_to_string(config).expect("config");
        assert!(!contents.contains("custom"));
        fs::remove_dir_all(home).expect("cleanup");
    }
}
