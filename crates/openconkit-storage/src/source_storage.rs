//! Filesystem adapter for [`SourceStorage`]: immutable source vault.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use openconkit_application::{
    HomeLayout, ImportedSource, SourceImportPolicy, SourceStorage, SourceStorageError,
};
use openconkit_domain::{ProjectId, Sha256Hash};
use sha2::{Digest, Sha256};

use crate::permissions::{
    harden_directory, harden_file, harden_read_only_file, make_owner_writable,
};

/// Filesystem-backed source vault under the app home.
///
/// Copies are written atomically (temp file → rename) under
/// `projects/<id>/sources/`, never touching the user's original.
pub struct FsSourceStorage {
    home: PathBuf,
}

impl FsSourceStorage {
    /// Create a vault rooted at `home` (the app home directory).
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }
}

impl SourceStorage for FsSourceStorage {
    fn import(
        &self,
        project_id: &ProjectId,
        source: &Path,
        policy: &SourceImportPolicy,
    ) -> Result<ImportedSource, SourceStorageError> {
        let original_name = source.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
            SourceStorageError::InvalidFileName {
                name: source.display().to_string(),
            }
        })?;
        let safe_name = sanitize_filename(original_name).ok_or_else(|| {
            SourceStorageError::InvalidFileName {
                name: original_name.to_string(),
            }
        })?;
        let extension = source
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| format!(".{extension}"))
            .unwrap_or_default();
        if !policy.accepts_extension(&extension) {
            return Err(SourceStorageError::UnsupportedExtension { extension });
        }
        let source_metadata = fs::metadata(source).map_err(io_err)?;
        if !source_metadata.is_file() {
            return Err(SourceStorageError::NotRegularFile {
                path: source.display().to_string(),
            });
        }
        if source_metadata.len() > policy.max_file_size_bytes {
            return Err(SourceStorageError::FileTooLarge {
                actual_bytes: source_metadata.len(),
                max_bytes: policy.max_file_size_bytes,
            });
        }

        let sources_rel = HomeLayout::project_sources_dir(project_id);
        let sources_dir = self.home.join(path_from_rel(&sources_rel));
        fs::create_dir_all(&sources_dir).map_err(io_err)?;
        harden_directory(&sources_dir).map_err(io_err)?;
        ensure_confined(&self.home, &sources_dir)?;

        // Stream + hash into a temp file under app-home temp/, then rename.
        let temp_dir = self.home.join(path_from_rel(HomeLayout::TEMP_DIR));
        fs::create_dir_all(&temp_dir).map_err(io_err)?;
        harden_directory(&temp_dir).map_err(io_err)?;
        ensure_confined(&self.home, &temp_dir)?;
        let temp_path = temp_dir.join(format!(
            "import-{}-{safe_name}",
            crate::atomic::unique_temp_suffix()
        ));

        let (sha256, size_bytes) = match copy_hashed(source, &temp_path, policy.max_file_size_bytes)
        {
            Ok(result) => result,
            Err(err) => {
                let _ = fs::remove_file(&temp_path);
                return Err(err);
            }
        };

        // Content-addressed subdirectory keeps collisions rare and makes
        // duplicate detection cheap at the filesystem level.
        let hash_prefix = &sha256.as_str()[..2];
        let dest_dir = sources_dir.join(hash_prefix);
        fs::create_dir_all(&dest_dir).map_err(io_err)?;
        harden_directory(&dest_dir).map_err(io_err)?;
        ensure_confined(&self.home, &dest_dir)?;
        let dest_path = unique_dest(&dest_dir, &safe_name)?;

        fs::rename(&temp_path, &dest_path).map_err(|err| {
            let _ = fs::remove_file(&temp_path);
            io_err(err)
        })?;

        // Make the stored copy read-only where the OS allows it — defence
        // in depth against accidental in-place edits.
        if let Err(err) = harden_read_only_file(&dest_path) {
            let _ = make_owner_writable(&dest_path);
            let _ = fs::remove_file(&dest_path);
            return Err(io_err(err));
        }

        let stored_relative_path = format!(
            "{sources_rel}/{hash_prefix}/{}",
            dest_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&safe_name)
        );

        Ok(ImportedSource {
            sha256,
            size_bytes,
            stored_relative_path,
        })
    }

    fn discard(&self, imported: &ImportedSource) -> Result<(), SourceStorageError> {
        let relative = validate_managed_source_path(&imported.stored_relative_path)?;
        let path = self.home.join(relative);
        if !path.exists() {
            return Ok(());
        }
        ensure_confined(&self.home, &path)?;
        make_owner_writable(&path).map_err(io_err)?;
        fs::remove_file(path).map_err(io_err)
    }
}

fn validate_managed_source_path(raw: &str) -> Result<PathBuf, SourceStorageError> {
    let path = path_from_rel(raw);
    let components: Vec<_> = path.components().collect();
    let valid = components.len() == 5
        && components[0].as_os_str() == "projects"
        && components[2].as_os_str() == "sources"
        && components
            .iter()
            .all(|component| matches!(component, std::path::Component::Normal(_)));
    if valid {
        Ok(path)
    } else {
        Err(SourceStorageError::InvalidFileName {
            name: raw.to_string(),
        })
    }
}

fn ensure_confined(home: &Path, path: &Path) -> Result<(), SourceStorageError> {
    let canonical_home = fs::canonicalize(home).map_err(io_err)?;
    let canonical_path = fs::canonicalize(path).map_err(io_err)?;
    if canonical_path.starts_with(&canonical_home) {
        Ok(())
    } else {
        Err(SourceStorageError::Io {
            message: format!("managed source path escaped app home: {}", path.display()),
        })
    }
}

fn copy_hashed(
    source: &Path,
    dest: &Path,
    max_file_size_bytes: u64,
) -> Result<(Sha256Hash, u64), SourceStorageError> {
    let mut input = File::open(source).map_err(io_err)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .map_err(io_err)?;
    harden_file(dest).map_err(io_err)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut size_bytes = 0u64;
    loop {
        let read = io::Read::read(&mut input, &mut buffer).map_err(io_err)?;
        if read == 0 {
            break;
        }
        let next_size =
            size_bytes
                .checked_add(read as u64)
                .ok_or(SourceStorageError::FileTooLarge {
                    actual_bytes: u64::MAX,
                    max_bytes: max_file_size_bytes,
                })?;
        if next_size > max_file_size_bytes {
            return Err(SourceStorageError::FileTooLarge {
                actual_bytes: next_size,
                max_bytes: max_file_size_bytes,
            });
        }
        output.write_all(&buffer[..read]).map_err(io_err)?;
        hasher.update(&buffer[..read]);
        size_bytes = next_size;
    }
    output.flush().map_err(io_err)?;
    output.sync_all().map_err(io_err)?;
    let digest: [u8; 32] = hasher.finalize().into();
    Ok((Sha256Hash::from_bytes(digest), size_bytes))
}

fn unique_dest(dir: &Path, safe_name: &str) -> Result<PathBuf, SourceStorageError> {
    let candidate = dir.join(safe_name);
    if !candidate.exists() {
        return Ok(candidate);
    }
    let stem = Path::new(safe_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = Path::new(safe_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    for n in 2..10_000 {
        let candidate = dir.join(format!("{stem}-{n}{ext}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(SourceStorageError::Io {
        message: format!("could not allocate unique name for {safe_name}"),
    })
}

/// Sanitize a user-supplied filename: strip path components, reject empty /
/// reserved names, keep only a conservative character set.
fn sanitize_filename(name: &str) -> Option<String> {
    let base = Path::new(name).file_name().and_then(|n| n.to_str())?.trim();
    if base.is_empty() || base == "." || base == ".." {
        return None;
    }
    let mut out = String::with_capacity(base.len());
    for ch in base.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | ' ') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim().trim_matches('.');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn path_from_rel(rel: &str) -> PathBuf {
    // HomeLayout paths use forward slashes; convert for the host OS.
    rel.split('/').collect()
}

fn io_err(err: io::Error) -> SourceStorageError {
    SourceStorageError::Io {
        message: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use openconkit_domain::ProjectId;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_home() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("openconkit-source-vault-{nanos}"));
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    fn xlsx_policy(max_file_size_bytes: u64) -> SourceImportPolicy {
        SourceImportPolicy {
            accepted_extensions: vec![".xls".into(), ".xlsx".into()],
            max_file_size_bytes,
        }
    }

    #[test]
    fn import_copies_hashes_and_never_touches_original() {
        let home = temp_home();
        let original_dir = home.join("user-files");
        fs::create_dir_all(&original_dir).expect("mkdir");
        let original = original_dir.join("boq.xlsx");
        fs::write(&original, b"fake-workbook-bytes").expect("write original");
        let original_meta_before = fs::metadata(&original).expect("meta");

        let vault = FsSourceStorage::new(&home);
        let project_id = ProjectId::new("tower-a").expect("slug");
        let imported = vault
            .import(&project_id, &original, &xlsx_policy(1024))
            .expect("import");

        assert_eq!(imported.size_bytes, b"fake-workbook-bytes".len() as u64);
        assert_eq!(imported.sha256.as_str().len(), 64);
        assert!(imported
            .stored_relative_path
            .starts_with("projects/tower-a/sources/"));
        assert!(imported.stored_relative_path.ends_with("boq.xlsx"));

        let stored = home.join(path_from_rel(&imported.stored_relative_path));
        assert!(stored.is_file());
        assert_eq!(
            fs::read(&stored).expect("read stored"),
            b"fake-workbook-bytes"
        );

        // Original untouched (content + still writable from the user's POV).
        assert_eq!(
            fs::read(&original).expect("read original"),
            b"fake-workbook-bytes"
        );
        let original_meta_after = fs::metadata(&original).expect("meta");
        assert_eq!(
            original_meta_before.modified().ok(),
            original_meta_after.modified().ok()
        );

        // Stored copy is content-addressed under the hash prefix.
        let prefix = &imported.sha256.as_str()[..2];
        assert!(imported.stored_relative_path.contains(prefix));

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn sanitize_rejects_traversal_and_empty() {
        assert!(sanitize_filename("../etc/passwd").is_some()); // becomes "passwd"
        assert_eq!(
            sanitize_filename("../etc/passwd").as_deref(),
            Some("passwd")
        );
        assert!(sanitize_filename("").is_none());
        assert!(sanitize_filename("..").is_none());
        assert!(sanitize_filename(".").is_none());
        assert_eq!(
            sanitize_filename("my boq (final).xlsx").as_deref(),
            Some("my boq _final_.xlsx")
        );
    }

    #[test]
    fn duplicate_imports_get_unique_names() {
        let home = temp_home();
        let original = home.join("user.xlsx");
        fs::write(&original, b"same").expect("write");
        let vault = FsSourceStorage::new(&home);
        let project_id = ProjectId::new("tower-a").expect("slug");
        let first = vault
            .import(&project_id, &original, &xlsx_policy(1024))
            .expect("first");
        // Same content + same name → unique dest under same hash prefix.
        let second = vault
            .import(&project_id, &original, &xlsx_policy(1024))
            .expect("second");
        assert_ne!(first.stored_relative_path, second.stored_relative_path);
        assert_eq!(first.sha256, second.sha256);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn discard_removes_only_managed_imports() {
        let home = temp_home();
        let original = home.join("user.xlsx");
        fs::write(&original, b"same").expect("write");
        let vault = FsSourceStorage::new(&home);
        let project_id = ProjectId::new("tower-a").expect("slug");
        let imported = vault
            .import(&project_id, &original, &xlsx_policy(1024))
            .expect("import");
        let stored = home.join(path_from_rel(&imported.stored_relative_path));
        assert!(stored.exists());

        vault.discard(&imported).expect("discard");
        assert!(!stored.exists());
        assert!(original.exists(), "original is never removed");

        let escaped = ImportedSource {
            stored_relative_path: "../user.xlsx".into(),
            ..imported
        };
        assert!(vault.discard(&escaped).is_err());
        assert!(original.exists());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn import_rejects_unsupported_extensions_before_copying() {
        let home = temp_home();
        let original = home.join("boq.pdf");
        fs::write(&original, b"not a workbook").expect("write");
        let vault = FsSourceStorage::new(&home);
        let project_id = ProjectId::new("tower-a").expect("slug");

        let error = vault
            .import(&project_id, &original, &xlsx_policy(1024))
            .expect_err("extension rejected");
        assert!(matches!(
            error,
            SourceStorageError::UnsupportedExtension { .. }
        ));
        assert!(!home.join("projects").exists());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn import_enforces_size_limit_without_leaving_temp_files() {
        let home = temp_home();
        let original = home.join("boq.xlsx");
        fs::write(&original, b"five!").expect("write");
        let vault = FsSourceStorage::new(&home);
        let project_id = ProjectId::new("tower-a").expect("slug");

        let error = vault
            .import(&project_id, &original, &xlsx_policy(4))
            .expect_err("size rejected");
        assert!(matches!(error, SourceStorageError::FileTooLarge { .. }));
        assert!(
            !home.join("temp").exists(),
            "preflight rejects before copying"
        );
        let _ = fs::remove_dir_all(&home);
    }
}
