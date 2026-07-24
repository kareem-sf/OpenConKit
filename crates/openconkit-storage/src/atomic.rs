//! Atomic file writes: temp file in the same directory, then rename.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::permissions::harden_file;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) fn unique_temp_suffix() -> String {
    let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    format!("{}-{sequence}", std::process::id())
}

/// Write `contents` to `path` atomically (temp-then-rename in the same dir).
///
/// Creates the parent directory if needed. On failure the temp file is
/// cleaned up when possible; the destination is left untouched.
pub fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path has no parent: {}", path.display()),
        )
    })?;
    fs::create_dir_all(parent)?;

    let file_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path has no file name: {}", path.display()),
        )
    })?;
    let temp_name = format!(".{file_name}.{}.tmp", unique_temp_suffix());
    let temp_path = parent.join(temp_name);

    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        harden_file(&temp_path)?;
        file.write_all(contents)?;
        file.sync_all()?;
        fs::rename(&temp_path, path)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("openconkit-atomic-{nanos}"));
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn atomic_write_creates_file_and_parent() {
        let dir = temp_dir();
        let path = dir.join("nested").join("settings.json");
        atomic_write(&path, b"{\"ok\":true}").expect("write");
        assert_eq!(fs::read_to_string(&path).expect("read"), "{\"ok\":true}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_write_replaces_existing() {
        let dir = temp_dir();
        let path = dir.join("file.txt");
        atomic_write(&path, b"v1").expect("write v1");
        atomic_write(&path, b"v2").expect("write v2");
        assert_eq!(fs::read_to_string(&path).expect("read"), "v2");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn concurrent_writes_never_share_a_temp_file() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let first_path = path.clone();
        let second_path = path.clone();
        let first = std::thread::spawn(move || atomic_write(&first_path, b"{\"value\":1}"));
        let second = std::thread::spawn(move || atomic_write(&second_path, b"{\"value\":2}"));

        first.join().expect("first thread").expect("first write");
        second.join().expect("second thread").expect("second write");
        let contents = fs::read_to_string(&path).expect("read");
        assert!(
            contents == "{\"value\":1}" || contents == "{\"value\":2}",
            "destination must contain one complete write: {contents}"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
