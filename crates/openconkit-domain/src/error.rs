//! Typed errors for the domain layer.

/// Stable machine-readable error codes shared with the frontend.
///
/// Codes are `SCREAMING_SNAKE` strings (e.g. `DOMAIN_INVALID_PROJECT_ID`).
/// The frontend maps each code to the i18n key `errors.<code>`; the
/// convention is documented repo-wide in `docs/architecture.md`.
pub trait ErrorCode {
    /// The stable code for this error.
    fn code(&self) -> &'static str;
}

/// Errors produced by domain invariants.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    /// A project id was empty or contained characters outside the allowed set.
    #[error("invalid project id: {0:?}")]
    InvalidProjectId(String),

    /// A project name was empty or only whitespace.
    #[error("project name must not be empty")]
    EmptyProjectName,

    /// A UUID-backed entity id could not be parsed.
    #[error("invalid {kind} id: {raw:?}")]
    InvalidId {
        /// Which id type failed to parse (e.g. `finding`).
        kind: &'static str,
        /// The raw input that failed validation.
        raw: String,
    },

    /// A SHA-256 hash was not exactly 64 lowercase ASCII hex characters.
    #[error("invalid SHA-256 hash: {0:?}")]
    InvalidSha256(String),

    /// A confidence value was outside the inclusive `0.0..=1.0` range.
    #[error("invalid confidence value: {value} (expected 0.0..=1.0)")]
    InvalidConfidence {
        /// The offending value.
        value: f64,
    },

    /// A cell reference was not a valid A1-style reference.
    #[error("invalid cell reference: {0:?}")]
    InvalidCellRef(String),

    /// A cell range was not of the form `<CellRef>:<CellRef>`.
    #[error("invalid cell range: {0:?}")]
    InvalidCellRange(String),

    /// A currency code was not exactly 3 uppercase ASCII letters.
    #[error("invalid currency code: {0:?}")]
    InvalidCurrency(String),

    /// A stored path was not a safe relative path (rooted, prefixed, or
    /// containing `..` components).
    #[error("invalid relative path: {0:?}")]
    InvalidRelativePath(String),
}

impl ErrorCode for DomainError {
    fn code(&self) -> &'static str {
        match self {
            DomainError::InvalidProjectId(_) => "DOMAIN_INVALID_PROJECT_ID",
            DomainError::EmptyProjectName => "DOMAIN_EMPTY_PROJECT_NAME",
            DomainError::InvalidId { .. } => "DOMAIN_INVALID_ID",
            DomainError::InvalidSha256(_) => "DOMAIN_INVALID_SHA256",
            DomainError::InvalidConfidence { .. } => "DOMAIN_INVALID_CONFIDENCE",
            DomainError::InvalidCellRef(_) => "DOMAIN_INVALID_CELL_REF",
            DomainError::InvalidCellRange(_) => "DOMAIN_INVALID_CELL_RANGE",
            DomainError::InvalidCurrency(_) => "DOMAIN_INVALID_CURRENCY",
            DomainError::InvalidRelativePath(_) => "DOMAIN_INVALID_RELATIVE_PATH",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable_screaming_snake() {
        let cases: [(DomainError, &str); 9] = [
            (
                DomainError::InvalidProjectId("x".into()),
                "DOMAIN_INVALID_PROJECT_ID",
            ),
            (DomainError::EmptyProjectName, "DOMAIN_EMPTY_PROJECT_NAME"),
            (
                DomainError::InvalidId {
                    kind: "finding",
                    raw: "x".into(),
                },
                "DOMAIN_INVALID_ID",
            ),
            (
                DomainError::InvalidSha256("x".into()),
                "DOMAIN_INVALID_SHA256",
            ),
            (
                DomainError::InvalidConfidence { value: 2.0 },
                "DOMAIN_INVALID_CONFIDENCE",
            ),
            (
                DomainError::InvalidCellRef("x".into()),
                "DOMAIN_INVALID_CELL_REF",
            ),
            (
                DomainError::InvalidCellRange("x".into()),
                "DOMAIN_INVALID_CELL_RANGE",
            ),
            (
                DomainError::InvalidCurrency("x".into()),
                "DOMAIN_INVALID_CURRENCY",
            ),
            (
                DomainError::InvalidRelativePath("x".into()),
                "DOMAIN_INVALID_RELATIVE_PATH",
            ),
        ];
        for (err, expected) in cases {
            assert_eq!(err.code(), expected);
            assert!(
                expected
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
                "{expected} is not SCREAMING_SNAKE"
            );
        }
    }
}
