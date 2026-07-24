//! Safe relative-path validation shared by stored source paths and export
//! paths. Paths are validated syntactically (component-wise, independent of
//! the host OS) so a path accepted on one platform is accepted on all.

use crate::DomainError;

/// Validate that `raw` is a safe relative path: non-empty, not rooted, no
/// drive/UNC prefix, and no `.`/`..` components. Both `/` and `\` are
/// treated as separators.
pub(crate) fn validate_relative_path(raw: &str) -> Result<(), DomainError> {
    let reject = || DomainError::InvalidRelativePath(raw.to_string());

    if raw.is_empty() || raw != raw.trim() {
        return Err(reject());
    }
    // Rooted paths: `/x`, `\x`, and UNC `\\server\share`.
    if raw.starts_with(['/', '\\']) {
        return Err(reject());
    }
    // Drive prefix, e.g. `C:\x` or `C:/x`.
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(reject());
    }
    for component in raw.split(['/', '\\']) {
        if component.is_empty() || component == "." || component == ".." {
            return Err(reject());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_relative_paths() {
        for ok in [
            "sources/abc.xlsx",
            "report.xlsx",
            "a/b/c-d_e f.pdf",
            "عربي/ملف.xlsx",
        ] {
            assert!(validate_relative_path(ok).is_ok(), "{ok} should be valid");
        }
    }

    #[test]
    fn rejects_traversal_rooted_and_prefixed_paths() {
        for bad in [
            "",
            " ",
            "../x",
            "a/../../b",
            "a/./b",
            "/abs/path",
            "\\rooted",
            "\\\\server\\share",
            "C:\\Users\\x",
            "c:/temp/x",
            "a//b",
            "trail/",
        ] {
            assert!(
                validate_relative_path(bad).is_err(),
                "{bad} should be rejected"
            );
        }
    }
}
