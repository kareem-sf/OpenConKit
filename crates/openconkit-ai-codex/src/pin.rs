//! The pinned Codex release, read from `tools/codex-version.json`.
//!
//! The manifest is embedded at compile time so the binary and the pin can
//! never drift apart. `scripts/fetch-codex.mjs` (Codex integration phase)
//! downloads the matching release and fills in checksums.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::CodexError;

/// Embedded copy of `tools/codex-version.json`.
const MANIFEST: &str = include_str!("../../../tools/codex-version.json");

/// A pinned Codex app-server release.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexPin {
    /// Semantic version, e.g. `0.145.0`.
    pub version: String,
    /// GitHub release tag, e.g. `rust-v0.145.0`.
    #[serde(rename = "releaseTag")]
    pub release_tag: String,
    /// Official license, notice, and stable protocol schema resources.
    pub resources: BTreeMap<String, CodexResourcePin>,
    /// Official release assets accepted for each supported build target.
    pub targets: BTreeMap<String, CodexTargetPin>,
}

/// One source-controlled official resource tied to the pinned Codex tag.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexResourcePin {
    /// Repository-relative path at the pinned official tag.
    #[serde(rename = "sourcePath")]
    pub source_path: String,
    /// Safe packaged filename.
    pub output: String,
    /// Exact byte length.
    pub size: u64,
    /// SHA-256 of the resource bytes.
    pub sha256: String,
}

/// One verified official app-server release artifact.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexTargetPin {
    /// GitHub release asset name.
    pub asset: String,
    /// Exact regular-file entry expected inside the archive.
    #[serde(rename = "archiveEntry")]
    pub archive_entry: String,
    /// Exact compressed asset size.
    #[serde(rename = "assetSize")]
    pub asset_size: u64,
    /// SHA-256 of the compressed release asset.
    pub sha256: String,
}

/// Parse the embedded manifest.
pub fn pinned_release() -> Result<CodexPin, CodexError> {
    serde_json::from_str(MANIFEST).map_err(|_| CodexError::Manifest)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn manifest_matches_pinned_release() {
        let pin = pinned_release().expect("parses");
        assert_eq!(pin.version, "0.145.0");
        assert_eq!(pin.release_tag, "rust-v0.145.0");
        assert_eq!(pin.resources.len(), 3);
        assert_eq!(pin.targets.len(), 4);
        let schema = pin
            .resources
            .get("appServerSchema")
            .expect("schema resource");
        assert_eq!(schema.size, 491_906);
        assert_eq!(schema.sha256.len(), 64);
        let windows = pin
            .targets
            .get("x86_64-pc-windows-msvc")
            .expect("windows target");
        assert_eq!(
            windows.asset,
            "codex-app-server-x86_64-pc-windows-msvc.exe.tar.gz"
        );
        assert_eq!(windows.sha256.len(), 64);
        assert!(!windows.sha256.contains("TO_FILL"));
    }
}
