//! The pinned Codex release, read from `tools/codex-version.json`.
//!
//! The manifest is embedded at compile time so the binary and the pin can
//! never drift apart. `scripts/fetch-codex.mjs` (Codex integration phase)
//! downloads the matching release and fills in checksums.

use serde::Deserialize;

use crate::CodexError;

/// Embedded copy of `tools/codex-version.json`.
const MANIFEST: &str = include_str!("../../../tools/codex-version.json");

/// A pinned Codex app-server release.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CodexPin {
    /// Semantic version, e.g. `0.145.0`.
    pub version: String,
    /// GitHub release tag, e.g. `rust-v0.145.0`.
    #[serde(rename = "releaseTag")]
    pub release_tag: String,
}

/// Parse the embedded manifest.
pub fn pinned_release() -> Result<CodexPin, CodexError> {
    Ok(serde_json::from_str(MANIFEST)?)
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
    }
}
