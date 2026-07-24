//! Deterministic report generation for OpenConKit tools.
//!
//! Reports are rendered from a fully prepared, localized [`ReportDocument`].
//! The XLSX and PDF writers never read a source workbook and publish only new
//! files, preserving the product's source-immutability invariant.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod model;
#[cfg(feature = "pdf")]
mod pdf;
mod xlsx;

pub use model::{
    ReportDetection, ReportDocument, ReportEvidence, ReportFinding, ReportLabels, ReportMetadata,
    ReportPareto, ReportSummary,
};
#[cfg(feature = "pdf")]
pub use pdf::write_pdf_report;
pub use xlsx::write_xlsx_report;

/// Errors from report generation.
#[derive(Debug, thiserror::Error)]
pub enum ReportingError {
    #[error("report destination already exists: {0}")]
    AlreadyExists(String),

    #[error("report I/O failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("xlsx export failed: {0}")]
    Xlsx(#[from] rust_xlsxwriter::XlsxError),

    #[error("report data is invalid: {0}")]
    InvalidData(String),

    #[cfg(feature = "pdf")]
    #[error("pdf export failed: {0}")]
    Pdf(String),
}
