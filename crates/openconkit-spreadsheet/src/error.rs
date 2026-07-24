//! Typed ingestion failures suitable for safe UI mapping.

/// Errors from bounded spreadsheet ingestion.
#[derive(Debug, thiserror::Error)]
pub enum SpreadsheetError {
    #[error("workbook ingestion limit `{field}` must be greater than zero")]
    InvalidLimit { field: &'static str },
    #[error("only .xls and .xlsx workbooks are supported")]
    UnsupportedExtension,
    #[error("the selected source is not a regular file")]
    NotRegularFile,
    #[error("failed to read workbook metadata: {message}")]
    Io { message: String },
    #[error("workbook is {actual_bytes} bytes; limit is {max_bytes} bytes")]
    FileTooLarge { actual_bytes: u64, max_bytes: u64 },
    #[error("XLSX archive contains {actual} entries; limit is {max}")]
    TooManyArchiveEntries { actual: usize, max: usize },
    #[error("XLSX archive entry `{entry}` is encrypted")]
    EncryptedArchiveEntry { entry: String },
    #[error("XLSX archive entry has an unsafe path: `{entry}`")]
    UnsafeArchiveEntry { entry: String },
    #[error(
        "XLSX archive entry `{entry}` expands to {actual_bytes} bytes; limit is {max_bytes} bytes"
    )]
    ArchiveEntryTooLarge {
        entry: String,
        actual_bytes: u64,
        max_bytes: u64,
    },
    #[error("XLSX archive declares {actual_bytes} uncompressed bytes; limit is {max_bytes} bytes")]
    ArchiveTooLarge { actual_bytes: u64, max_bytes: u64 },
    #[error(
        "XLSX archive entry `{entry}` has compression ratio above the configured {max_ratio}:1 limit"
    )]
    SuspiciousCompressionRatio { entry: String, max_ratio: u64 },
    #[error("failed to inspect XLSX archive: {message}")]
    Archive { message: String },
    #[error("workbook contains {actual} sheets; limit is {max}")]
    TooManySheets { actual: usize, max: usize },
    #[error("sheet `{sheet}` reaches row {actual}; configured maximum row coordinate is {max}")]
    TooManyRows {
        sheet: String,
        actual: u32,
        max: u32,
    },
    #[error(
        "sheet `{sheet}` reaches column {actual}; configured maximum column coordinate is {max}"
    )]
    TooManyColumns {
        sheet: String,
        actual: u32,
        max: u32,
    },
    #[error("sheet `{sheet}` contains {actual} merged regions; limit is {max}")]
    TooManyMergedRegions {
        sheet: String,
        actual: usize,
        max: usize,
    },
    #[error("workbook contains more than {max} retained cells")]
    TooManyCells { max: usize },
    #[error("cell `{cell}` contains {actual_bytes} text bytes; limit is {max_bytes}")]
    CellTextTooLarge {
        cell: String,
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("formula in `{cell}` contains {actual_bytes} bytes; limit is {max_bytes}")]
    FormulaTooLarge {
        cell: String,
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("workbook retained text exceeds the {max_bytes}-byte limit")]
    TotalTextTooLarge { max_bytes: usize },
    #[error("failed to parse workbook: {message}")]
    Parse { message: String },
    #[error("workbook ingestion was cancelled")]
    Cancelled,
}

impl SpreadsheetError {
    /// Stable machine-readable error code for IPC and localization.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidLimit { .. } => "INVALID_LIMIT",
            Self::UnsupportedExtension => "UNSUPPORTED_EXTENSION",
            Self::NotRegularFile => "NOT_REGULAR_FILE",
            Self::Io { .. } => "IO",
            Self::FileTooLarge { .. } => "FILE_TOO_LARGE",
            Self::TooManyArchiveEntries { .. } => "TOO_MANY_ARCHIVE_ENTRIES",
            Self::EncryptedArchiveEntry { .. } => "ENCRYPTED_WORKBOOK",
            Self::UnsafeArchiveEntry { .. } => "UNSAFE_ARCHIVE_ENTRY",
            Self::ArchiveEntryTooLarge { .. } => "ARCHIVE_ENTRY_TOO_LARGE",
            Self::ArchiveTooLarge { .. } => "ARCHIVE_TOO_LARGE",
            Self::SuspiciousCompressionRatio { .. } => "SUSPICIOUS_COMPRESSION_RATIO",
            Self::Archive { .. } => "INVALID_ARCHIVE",
            Self::TooManySheets { .. } => "TOO_MANY_SHEETS",
            Self::TooManyRows { .. } => "TOO_MANY_ROWS",
            Self::TooManyColumns { .. } => "TOO_MANY_COLUMNS",
            Self::TooManyMergedRegions { .. } => "TOO_MANY_MERGED_REGIONS",
            Self::TooManyCells { .. } => "TOO_MANY_CELLS",
            Self::CellTextTooLarge { .. } => "CELL_TEXT_TOO_LARGE",
            Self::FormulaTooLarge { .. } => "FORMULA_TOO_LARGE",
            Self::TotalTextTooLarge { .. } => "TOTAL_TEXT_TOO_LARGE",
            Self::Parse { .. } => "WORKBOOK_PARSE_FAILED",
            Self::Cancelled => "CANCELLED",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(SpreadsheetError::Cancelled.code(), "CANCELLED");
        assert_eq!(
            SpreadsheetError::UnsupportedExtension.code(),
            "UNSUPPORTED_EXTENSION"
        );
        assert_eq!(
            SpreadsheetError::ArchiveTooLarge {
                actual_bytes: 2,
                max_bytes: 1,
            }
            .code(),
            "ARCHIVE_TOO_LARGE"
        );
    }
}
