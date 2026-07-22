//! Sidecar binary layout.
//!
//! Codex app-server binaries are staged at build time under
//! `crates/openconkit-desktop/binaries/` (never committed; see `.gitignore`)
//! following the Tauri external-binary naming convention:
//! `<name>-<target-triple>[.exe]`.

/// Name of the sidecar binary for a Rust target triple, following the Tauri
/// external-binary convention.
pub fn sidecar_binary_name(target_triple: &str) -> String {
    if target_triple.contains("windows") {
        format!("codex-app-server-{target_triple}.exe")
    } else {
        format!("codex-app-server-{target_triple}")
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn windows_triples_get_exe_suffix() {
        assert_eq!(
            sidecar_binary_name("x86_64-pc-windows-msvc"),
            "codex-app-server-x86_64-pc-windows-msvc.exe"
        );
    }

    #[test]
    fn unix_triples_have_no_suffix() {
        assert_eq!(
            sidecar_binary_name("aarch64-apple-darwin"),
            "codex-app-server-aarch64-apple-darwin"
        );
    }
}
