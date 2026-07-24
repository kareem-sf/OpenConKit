//! Metadata-only Codex protocol diagnostics.
//!
//! Raw JSON, parameters, results, stderr text, paths, workbook values,
//! credentials, and upstream error messages are never written. This module
//! receives a parsed envelope only to derive bounded routing metadata.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use jiff::Timestamp;
use serde::Serialize;
use serde_json::Value;

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
const ROTATION_COUNT: u8 = 3;
const MAX_METHOD_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Direction {
    Inbound,
    Outbound,
}

#[derive(Serialize)]
struct ProtocolMetadata<'a> {
    timestamp: Timestamp,
    direction: Direction,
    kind: &'a str,
    method: Option<&'a str>,
    request_id: Option<u64>,
    status: &'a str,
    bytes: usize,
}

/// Synchronous writes are tiny and guarded so concurrent stdout/request tasks
/// cannot interleave JSONL records.
pub(crate) struct ProtocolLogger {
    path: PathBuf,
    write_lock: Mutex<()>,
}

impl ProtocolLogger {
    pub(crate) fn new(path: &Path) -> std::io::Result<Self> {
        let parent = path.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "log path has no parent")
        })?;
        fs::create_dir_all(parent)?;
        reject_symlink(parent)?;
        regular_file_if_exists(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            write_lock: Mutex::new(()),
        })
    }

    pub(crate) fn record_message(&self, direction: Direction, value: &Value, bytes: usize) {
        let method = value
            .get("method")
            .and_then(Value::as_str)
            .filter(|method| safe_method(method));
        let request_id = value.get("id").and_then(Value::as_u64);
        let (kind, status) = if method.is_some() && value.get("id").is_some() {
            ("request", "observed")
        } else if method.is_some() {
            ("notification", "observed")
        } else if value.get("result").is_some() {
            ("response", "ok")
        } else if value.get("error").is_some() {
            ("response", "error")
        } else {
            ("invalid", "rejected")
        };
        self.record(ProtocolMetadata {
            timestamp: Timestamp::now(),
            direction,
            kind,
            method,
            request_id,
            status,
            bytes,
        });
    }

    pub(crate) fn record_malformed(&self, direction: Direction, bytes: usize) {
        self.record(ProtocolMetadata {
            timestamp: Timestamp::now(),
            direction,
            kind: "malformed",
            method: None,
            request_id: None,
            status: "rejected",
            bytes,
        });
    }

    pub(crate) fn record_stderr(&self, bytes: usize) {
        self.record(ProtocolMetadata {
            timestamp: Timestamp::now(),
            direction: Direction::Inbound,
            kind: "stderr",
            method: None,
            request_id: None,
            status: "discarded",
            bytes,
        });
    }

    fn record(&self, event: ProtocolMetadata<'_>) {
        let Ok(_guard) = self.write_lock.lock() else {
            return;
        };
        let Ok(mut encoded) = serde_json::to_vec(&event) else {
            return;
        };
        encoded.push(b'\n');
        if rotate_if_needed(&self.path, encoded.len() as u64).is_err() {
            return;
        }
        let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        else {
            return;
        };
        if harden_file(&self.path).is_err() {
            return;
        }
        let _ = file.write_all(&encoded);
        let _ = file.flush();
    }
}

fn safe_method(method: &str) -> bool {
    !method.is_empty()
        && method.len() <= MAX_METHOD_BYTES
        && method
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
}

fn rotate_if_needed(path: &Path, incoming: u64) -> std::io::Result<()> {
    let current = fs::metadata(path).map_or(0, |metadata| metadata.len());
    if current.saturating_add(incoming) <= MAX_LOG_BYTES {
        return Ok(());
    }
    for index in (1..=ROTATION_COUNT).rev() {
        let destination = rotated_path(path, index);
        if regular_file_if_exists(&destination)? {
            fs::remove_file(&destination)?;
        }
        let source = if index == 1 {
            path.to_path_buf()
        } else {
            rotated_path(path, index - 1)
        };
        if regular_file_if_exists(&source)? {
            fs::rename(source, destination)?;
        }
    }
    Ok(())
}

fn rotated_path(path: &Path, index: u8) -> PathBuf {
    path.with_extension(format!("jsonl.{index}"))
}

fn reject_symlink(path: &Path) -> std::io::Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "protocol log path must not be a symlink",
        ));
    }
    Ok(())
}

fn regular_file_if_exists(path: &Path) -> std::io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "protocol log path must not be a symlink",
        )),
        Ok(metadata) if !metadata.is_file() => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "protocol log path must be a regular file",
        )),
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn harden_file(path: &Path) -> std::io::Result<()> {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::*;

    fn temporary_log() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("openconkit-protocol-log-{nanos}"))
            .join("codex-protocol.jsonl")
    }

    #[test]
    fn raw_params_results_and_stderr_are_never_written() {
        let path = temporary_log();
        let logger = ProtocolLogger::new(&path).expect("logger");
        logger.record_message(
            Direction::Outbound,
            &json!({
                "id": 7,
                "method": "turn/start",
                "params": {
                    "prompt": "rate for A101 is 42",
                    "token": "secret@example.com"
                }
            }),
            123,
        );
        logger.record_message(
            Direction::Inbound,
            &json!({
                "id": 7,
                "result": {"assistant": "A101 is risky"}
            }),
            77,
        );
        logger.record_stderr("credential leaked here".len());
        let contents = fs::read_to_string(&path).expect("read");
        assert!(contents.contains("\"method\":\"turn/start\""));
        assert!(contents.contains("\"request_id\":7"));
        for prohibited in [
            "A101",
            "42",
            "secret@example.com",
            "risky",
            "credential leaked here",
            "params",
            "result",
        ] {
            assert!(!contents.contains(prohibited), "leaked {prohibited}");
        }
        fs::remove_dir_all(path.parent().expect("parent")).expect("cleanup");
    }

    #[test]
    fn attacker_controlled_method_is_not_logged() {
        let path = temporary_log();
        let logger = ProtocolLogger::new(&path).expect("logger");
        logger.record_message(
            Direction::Inbound,
            &json!({"method": "secret@example.com\nA101", "params": {}}),
            40,
        );
        let contents = fs::read_to_string(&path).expect("read");
        assert!(!contents.contains("secret@example.com"));
        assert!(contents.contains("\"method\":null"));
        fs::remove_dir_all(path.parent().expect("parent")).expect("cleanup");
    }

    #[test]
    fn oversized_log_rotates_without_overwriting_non_log_paths() {
        let path = temporary_log();
        let logger = ProtocolLogger::new(&path).expect("logger");
        fs::write(&path, vec![b'x'; MAX_LOG_BYTES as usize]).expect("seed");
        logger.record_malformed(Direction::Inbound, 10);
        assert!(rotated_path(&path, 1).is_file());
        let current = fs::read_to_string(&path).expect("current");
        assert!(current.contains("\"kind\":\"malformed\""));
        fs::remove_dir_all(path.parent().expect("parent")).expect("cleanup");
    }
}
