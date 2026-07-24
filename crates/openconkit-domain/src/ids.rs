//! Entity identifiers and content hashes.
//!
//! UUID-backed ids are strongly typed per entity so a [`SourceRevisionId`]
//! can never be passed where a [`FindingId`] is expected. `ProjectId` (a
//! human-meaningful slug) stays in [`crate::project`].

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use crate::DomainError;

macro_rules! uuid_id {
    ($(#[$meta:meta])* $name:ident, $kind:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
        pub struct $name(uuid::Uuid);

        impl $name {
            /// Generate a new random (v4) id.
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4())
            }

            /// Wrap an existing UUID.
            pub fn from_uuid(id: uuid::Uuid) -> Self {
                Self(id)
            }

            /// Parse the canonical string form (e.g. from storage or IPC).
            pub fn parse(raw: &str) -> Result<Self, DomainError> {
                uuid::Uuid::parse_str(raw).map(Self).map_err(|_| {
                    DomainError::InvalidId {
                        kind: $kind,
                        raw: raw.to_string(),
                    }
                })
            }

            /// The inner UUID.
            pub fn as_uuid(&self) -> uuid::Uuid {
                self.0
            }
        }

        // `Default` generates a fresh random id; it exists so `new()` does
        // not trip `clippy::new_without_default`, not to imply a zero value.
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

uuid_id!(
    /// Identifier of an imported source workbook revision.
    SourceRevisionId,
    "source_revision"
);
uuid_id!(
    /// Identifier of an analysis run.
    AnalysisRunId,
    "analysis_run"
);
uuid_id!(
    /// Identifier of a single finding.
    FindingId,
    "finding"
);
uuid_id!(
    /// Identifier of a generated export artifact.
    ExportId,
    "export"
);
uuid_id!(
    /// Identifier of an AI analysis record.
    AiAnalysisId,
    "ai_analysis"
);

/// A SHA-256 content hash, stored as exactly 64 lowercase ASCII hex chars.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, TS)]
pub struct Sha256Hash(String);

impl Sha256Hash {
    /// Create from a hex string, enforcing 64 lowercase ASCII hex chars.
    pub fn from_hex(raw: &str) -> Result<Self, DomainError> {
        let valid = raw.len() == 64
            && raw
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if valid {
            Ok(Self(raw.to_string()))
        } else {
            Err(DomainError::InvalidSha256(raw.to_string()))
        }
    }

    /// Create from a raw 32-byte digest, hex-encoding it.
    pub fn from_bytes(digest: [u8; 32]) -> Self {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(64);
        for byte in digest {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
        Self(out)
    }

    /// Borrow the hex string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Sha256Hash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::from_hex(&raw).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Sha256Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn uuid_ids_generate_parse_and_display_round_trip() {
        let id = FindingId::new();
        let text = id.to_string();
        let parsed = FindingId::parse(&text).expect("fresh id parses");
        assert_eq!(parsed, id);
        assert_eq!(parsed.as_uuid(), id.as_uuid());
        assert_eq!(FindingId::from_uuid(id.as_uuid()), id);
    }

    #[test]
    fn uuid_id_kinds_are_distinct_types() {
        // Different kinds must not be interchangeable; parse errors carry
        // the kind for diagnostics.
        let err = SourceRevisionId::parse("not-a-uuid").expect_err("must fail");
        assert!(matches!(
            err,
            DomainError::InvalidId {
                kind: "source_revision",
                ref raw,
            } if raw == "not-a-uuid"
        ));
    }

    #[test]
    fn uuid_id_parse_accepts_mixed_case_and_rejects_garbage() {
        let id = AnalysisRunId::new();
        let upper = id.to_string().to_ascii_uppercase();
        assert_eq!(AnalysisRunId::parse(&upper).expect("parses"), id);
        assert!(AnalysisRunId::parse("").is_err());
        assert!(AnalysisRunId::parse("1234").is_err());
    }

    #[test]
    fn sha256_accepts_64_lowercase_hex() {
        let hex = "a".repeat(64);
        let hash = Sha256Hash::from_hex(&hex).expect("valid hash");
        assert_eq!(hash.as_str(), hex);
        assert_eq!(hash.to_string(), hex);
    }

    #[test]
    fn sha256_rejects_bad_input() {
        assert!(Sha256Hash::from_hex(&"a".repeat(63)).is_err());
        assert!(Sha256Hash::from_hex(&"a".repeat(65)).is_err());
        assert!(Sha256Hash::from_hex(&"A".repeat(64)).is_err());
        assert!(Sha256Hash::from_hex(&"g".repeat(64)).is_err());
        assert!(Sha256Hash::from_hex("").is_err());
    }

    #[test]
    fn sha256_from_bytes_hex_encodes() {
        let mut digest = [0u8; 32];
        digest[0] = 0x0f;
        digest[31] = 0xff;
        let hash = Sha256Hash::from_bytes(digest);
        assert_eq!(
            hash.as_str(),
            "0f000000000000000000000000000000000000000000000000000000000000ff"
        );
        // Round-trips through from_hex.
        assert_eq!(
            Sha256Hash::from_hex(hash.as_str()).expect("encodes validly"),
            hash
        );
    }

    #[test]
    fn sha256_serde_is_a_plain_string() {
        let hash = Sha256Hash::from_bytes([0xab; 32]);
        let json = serde_json::to_string(&hash).expect("serialize");
        assert_eq!(json, format!("\"{}\"", "ab".repeat(32)));
        let back: Sha256Hash = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, hash);
    }

    #[test]
    fn sha256_deserialization_enforces_hash_invariant() {
        let parsed: Result<Sha256Hash, _> = serde_json::from_str("\"not-a-hash\"");
        assert!(parsed.is_err());
    }

    #[test]
    fn ids_map_to_ts_string() {
        let cfg = ts_rs::Config::default();
        let finding_id = <FindingId as TS>::decl(&cfg);
        assert!(finding_id.contains("= string"), "{finding_id}");
        let hash = <Sha256Hash as TS>::decl(&cfg);
        assert!(hash.contains("= string"), "{hash}");
    }
}
