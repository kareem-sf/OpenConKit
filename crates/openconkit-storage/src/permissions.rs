//! Restrictive filesystem permissions for app-owned data.
//!
//! Unix permission bits are made explicit because "read-only" otherwise
//! commonly becomes `0444`, exposing workbook copies to other local users.
//! Windows relies on the per-user home directory's inherited ACL; the
//! read-only file attribute is handled separately for immutable sources.

use std::fs;
use std::path::Path;

/// Restrict an app-owned directory to its owner where Unix modes exist.
pub(crate) fn harden_directory(path: &Path) -> std::io::Result<()> {
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

/// Restrict a mutable app-owned file to its owner where Unix modes exist.
pub(crate) fn harden_file(path: &Path) -> std::io::Result<()> {
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

/// Make an immutable imported source owner-readable only on Unix.
pub(crate) fn harden_read_only_file(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o400))?;
    }
    #[cfg(not(unix))]
    {
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

/// Restore owner write access before removing an immutable source.
///
/// On Windows, `set_readonly(false)` only clears the DOS read-only attribute;
/// access remains constrained by the per-user app-home ACL. Clippy's warning
/// concerns Unix mode bits, which are handled by the explicit `0o600` branch.
#[cfg_attr(windows, allow(clippy::permissions_set_readonly_false))]
pub(crate) fn make_owner_writable(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    {
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn unix_modes_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("openconkit-permissions-{suffix}"));
        fs::create_dir_all(&dir).expect("mkdir");
        let file = dir.join("source.xlsx");
        fs::write(&file, b"workbook").expect("write");

        harden_directory(&dir).expect("directory permissions");
        harden_file(&file).expect("file permissions");
        assert_eq!(
            fs::metadata(&dir).expect("metadata").permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&file).expect("metadata").permissions().mode() & 0o777,
            0o600
        );

        harden_read_only_file(&file).expect("readonly");
        assert_eq!(
            fs::metadata(&file).expect("metadata").permissions().mode() & 0o777,
            0o400
        );
        make_owner_writable(&file).expect("writable");
        assert_eq!(
            fs::metadata(&file).expect("metadata").permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(&dir).expect("cleanup");
    }
}
