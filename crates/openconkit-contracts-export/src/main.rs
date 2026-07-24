//! OpenConKit contracts exporter.
//!
//! Generates TypeScript bindings for every `ts_rs::TS`-deriving type that
//! crosses the Rust↔TypeScript boundary (domain entities, application
//! settings/bootstrap DTOs, and the tool SDK contract) into
//! `packages/contracts/src/generated/`, and writes a barrel `index.ts`.
//!
//! Generated files are committed to the repository and drift-checked in CI
//! (see `docs/adr/0005-ts-rs-generated-contracts.md`).
//!
//! Usage:
//! - `cargo run -p openconkit-contracts-export`            write bindings
//! - `cargo run -p openconkit-contracts-export -- --check` exit 1 on drift
//!
//! The export surface is explicit: adding a new contract type requires
//! listing it here so the change is reviewable.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use ts_rs::TS;

use openconkit_application::{
    AdvancedSettings, AiAccountSnapshot, AiLoginChallenge, AiLoginMode, AiPlanType,
    AiRateLimitSnapshot, AiRateLimitWindow, AiReviewScope, AiRuntimeStatus, AnalysisTolerances,
    AppSettings, AvailableUpdate, BootstrapStatus, IpcError, Language, PrivacySettings, RunDetails,
    RunHistoryEntry, SettingsPatch, Theme, UpdateChannel, UpdateChannelState, UpdateCheckResult,
    UpdateProgressEvent, UpdateProgressPhase,
};
use openconkit_domain::{
    AiAnalysis, AiAnalysisId, AiAnalysisLanguage, AiAnalysisStatus, AiGroundingStatus,
    AiValidationStatus, AnalysisRun, AnalysisRunId, CellRange, CellRef, ClassifiedRow, ColumnRole,
    ColumnRoleAssignment, Confidence, Currency, DetectedTable, Evidence, ExportId, ExportKind,
    ExportRecord, Finding, FindingCategory, FindingId, FindingOrigin, MoneyAmount, Project,
    ProjectId, ProjectMetadata, RowClassification, RunStatus, Severity, Sha256Hash, SheetInventory,
    SheetVisibility, SourceRevision, SourceRevisionId, WorkbookDiagnostics,
};
use openconkit_tool_boq_inspector::{
    BoqAiPrioritizedRisk, BoqAiPriority, BoqAiReview, BoqInspectorInput, BoqInspectorOutput,
    BoqInspectorSettings, BoqInspectorSummary, BoqNormalizedFact, BoqNormalizedRow, ParetoAnalysis,
};
use openconkit_tool_sdk::{
    AiCapability, ExportedArtifact, InputCapabilities, ToolManifest, ToolNavItem, ToolPermissions,
    ToolProgress, ToolProgressEvent, ToolRunContext, ToolRunResponse,
};

/// Repository root, derived from the crate manifest directory
/// (`crates/openconkit-contracts-export` → two levels up).
fn repo_root() -> Result<PathBuf, String> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .map_err(|e| format!("repo root does not resolve: {e}"))
}

/// `packages/contracts/src/generated`.
fn generated_dir() -> Result<PathBuf, String> {
    Ok(repo_root()?.join("packages/contracts/src/generated"))
}

/// A contract type to export: its name (used as the TS module/file stem) and
/// an export function.
struct ContractType {
    name: &'static str,
    export: fn(&ts_rs::Config, &Path) -> Result<(), ts_rs::ExportError>,
}

fn export_one<T: TS + 'static>(_cfg: &ts_rs::Config, dir: &Path) -> Result<(), ts_rs::ExportError> {
    let cfg = ts_rs::Config::default().with_out_dir(dir);
    T::export_all(&cfg)
}

/// Every type whose bindings are part of the committed contract surface.
///
/// The list is explicit so adding a contract type is a reviewable change.
/// ts-rs writes each type to `<TypeName>.ts` (no `#[ts(export_to)]` is used
/// anywhere), so the type name doubles as the barrel module name.
fn contract_types() -> Vec<ContractType> {
    // openconkit-domain
    let domain = [
        ContractType {
            name: "AiAnalysis",
            export: export_one::<AiAnalysis>,
        },
        ContractType {
            name: "AiAnalysisId",
            export: export_one::<AiAnalysisId>,
        },
        ContractType {
            name: "AiAnalysisLanguage",
            export: export_one::<AiAnalysisLanguage>,
        },
        ContractType {
            name: "AiAnalysisStatus",
            export: export_one::<AiAnalysisStatus>,
        },
        ContractType {
            name: "AiGroundingStatus",
            export: export_one::<AiGroundingStatus>,
        },
        ContractType {
            name: "AiValidationStatus",
            export: export_one::<AiValidationStatus>,
        },
        ContractType {
            name: "AnalysisRun",
            export: export_one::<AnalysisRun>,
        },
        ContractType {
            name: "AnalysisRunId",
            export: export_one::<AnalysisRunId>,
        },
        ContractType {
            name: "CellRange",
            export: export_one::<CellRange>,
        },
        ContractType {
            name: "CellRef",
            export: export_one::<CellRef>,
        },
        ContractType {
            name: "ClassifiedRow",
            export: export_one::<ClassifiedRow>,
        },
        ContractType {
            name: "ColumnRole",
            export: export_one::<ColumnRole>,
        },
        ContractType {
            name: "ColumnRoleAssignment",
            export: export_one::<ColumnRoleAssignment>,
        },
        ContractType {
            name: "Confidence",
            export: export_one::<Confidence>,
        },
        ContractType {
            name: "Currency",
            export: export_one::<Currency>,
        },
        ContractType {
            name: "DetectedTable",
            export: export_one::<DetectedTable>,
        },
        ContractType {
            name: "Evidence",
            export: export_one::<Evidence>,
        },
        ContractType {
            name: "ExportId",
            export: export_one::<ExportId>,
        },
        ContractType {
            name: "ExportKind",
            export: export_one::<ExportKind>,
        },
        ContractType {
            name: "ExportRecord",
            export: export_one::<ExportRecord>,
        },
        ContractType {
            name: "Finding",
            export: export_one::<Finding>,
        },
        ContractType {
            name: "FindingCategory",
            export: export_one::<FindingCategory>,
        },
        ContractType {
            name: "FindingId",
            export: export_one::<FindingId>,
        },
        ContractType {
            name: "FindingOrigin",
            export: export_one::<FindingOrigin>,
        },
        ContractType {
            name: "MoneyAmount",
            export: export_one::<MoneyAmount>,
        },
        ContractType {
            name: "Project",
            export: export_one::<Project>,
        },
        ContractType {
            name: "ProjectId",
            export: export_one::<ProjectId>,
        },
        ContractType {
            name: "ProjectMetadata",
            export: export_one::<ProjectMetadata>,
        },
        ContractType {
            name: "RowClassification",
            export: export_one::<RowClassification>,
        },
        ContractType {
            name: "RunStatus",
            export: export_one::<RunStatus>,
        },
        ContractType {
            name: "Severity",
            export: export_one::<Severity>,
        },
        ContractType {
            name: "Sha256Hash",
            export: export_one::<Sha256Hash>,
        },
        ContractType {
            name: "SheetInventory",
            export: export_one::<SheetInventory>,
        },
        ContractType {
            name: "SheetVisibility",
            export: export_one::<SheetVisibility>,
        },
        ContractType {
            name: "SourceRevision",
            export: export_one::<SourceRevision>,
        },
        ContractType {
            name: "SourceRevisionId",
            export: export_one::<SourceRevisionId>,
        },
        ContractType {
            name: "WorkbookDiagnostics",
            export: export_one::<WorkbookDiagnostics>,
        },
    ];
    // openconkit-application
    let application = [
        ContractType {
            name: "AdvancedSettings",
            export: export_one::<AdvancedSettings>,
        },
        ContractType {
            name: "AiAccountSnapshot",
            export: export_one::<AiAccountSnapshot>,
        },
        ContractType {
            name: "AiLoginChallenge",
            export: export_one::<AiLoginChallenge>,
        },
        ContractType {
            name: "AiLoginMode",
            export: export_one::<AiLoginMode>,
        },
        ContractType {
            name: "AiPlanType",
            export: export_one::<AiPlanType>,
        },
        ContractType {
            name: "AiRateLimitSnapshot",
            export: export_one::<AiRateLimitSnapshot>,
        },
        ContractType {
            name: "AiRateLimitWindow",
            export: export_one::<AiRateLimitWindow>,
        },
        ContractType {
            name: "AiReviewScope",
            export: export_one::<AiReviewScope>,
        },
        ContractType {
            name: "AiRuntimeStatus",
            export: export_one::<AiRuntimeStatus>,
        },
        ContractType {
            name: "AnalysisTolerances",
            export: export_one::<AnalysisTolerances>,
        },
        ContractType {
            name: "AppSettings",
            export: export_one::<AppSettings>,
        },
        ContractType {
            name: "BootstrapStatus",
            export: export_one::<BootstrapStatus>,
        },
        ContractType {
            name: "Language",
            export: export_one::<Language>,
        },
        ContractType {
            name: "PrivacySettings",
            export: export_one::<PrivacySettings>,
        },
        ContractType {
            name: "RunDetails",
            export: export_one::<RunDetails>,
        },
        ContractType {
            name: "RunHistoryEntry",
            export: export_one::<RunHistoryEntry>,
        },
        ContractType {
            name: "IpcError",
            export: export_one::<IpcError>,
        },
        ContractType {
            name: "SettingsPatch",
            export: export_one::<SettingsPatch>,
        },
        ContractType {
            name: "Theme",
            export: export_one::<Theme>,
        },
        ContractType {
            name: "UpdateChannel",
            export: export_one::<UpdateChannel>,
        },
        ContractType {
            name: "UpdateChannelState",
            export: export_one::<UpdateChannelState>,
        },
        ContractType {
            name: "AvailableUpdate",
            export: export_one::<AvailableUpdate>,
        },
        ContractType {
            name: "UpdateCheckResult",
            export: export_one::<UpdateCheckResult>,
        },
        ContractType {
            name: "UpdateProgressEvent",
            export: export_one::<UpdateProgressEvent>,
        },
        ContractType {
            name: "UpdateProgressPhase",
            export: export_one::<UpdateProgressPhase>,
        },
    ];
    // openconkit-tool-sdk
    let tool_sdk = [
        ContractType {
            name: "AiCapability",
            export: export_one::<AiCapability>,
        },
        ContractType {
            name: "ExportedArtifact",
            export: export_one::<ExportedArtifact>,
        },
        ContractType {
            name: "InputCapabilities",
            export: export_one::<InputCapabilities>,
        },
        ContractType {
            name: "ToolManifest",
            export: export_one::<ToolManifest>,
        },
        ContractType {
            name: "ToolNavItem",
            export: export_one::<ToolNavItem>,
        },
        ContractType {
            name: "ToolPermissions",
            export: export_one::<ToolPermissions>,
        },
        ContractType {
            name: "ToolProgress",
            export: export_one::<ToolProgress>,
        },
        ContractType {
            name: "ToolProgressEvent",
            export: export_one::<ToolProgressEvent>,
        },
        ContractType {
            name: "ToolRunContext",
            export: export_one::<ToolRunContext>,
        },
        ContractType {
            name: "ToolRunResponse",
            export: export_one::<ToolRunResponse>,
        },
    ];
    // Compiled-in tool contracts
    let tool_boq = [
        ContractType {
            name: "BoqAiPrioritizedRisk",
            export: export_one::<BoqAiPrioritizedRisk>,
        },
        ContractType {
            name: "BoqAiPriority",
            export: export_one::<BoqAiPriority>,
        },
        ContractType {
            name: "BoqAiReview",
            export: export_one::<BoqAiReview>,
        },
        ContractType {
            name: "BoqInspectorInput",
            export: export_one::<BoqInspectorInput>,
        },
        ContractType {
            name: "BoqInspectorOutput",
            export: export_one::<BoqInspectorOutput>,
        },
        ContractType {
            name: "BoqInspectorSettings",
            export: export_one::<BoqInspectorSettings>,
        },
        ContractType {
            name: "BoqInspectorSummary",
            export: export_one::<BoqInspectorSummary>,
        },
        ContractType {
            name: "BoqNormalizedFact",
            export: export_one::<BoqNormalizedFact>,
        },
        ContractType {
            name: "BoqNormalizedRow",
            export: export_one::<BoqNormalizedRow>,
        },
        ContractType {
            name: "ParetoAnalysis",
            export: export_one::<ParetoAnalysis>,
        },
    ];
    let mut all = Vec::new();
    all.extend(domain);
    all.extend(application);
    all.extend(tool_sdk);
    all.extend(tool_boq);
    all
}

fn run_export(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // Drop prior generated .ts files so removed types don't linger. Keep
    // non-ts markers (e.g. .gitkeep) intact.
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "ts") {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    let cfg = ts_rs::Config::default();
    let types = contract_types();
    for ty in &types {
        (ty.export)(&cfg, dir).map_err(|err| format!("failed to export {}: {err}", ty.name))?;
    }
    write_barrel(dir, &types).map_err(|e| e.to_string())?;
    normalize_generated_files(dir)?;
    Ok(())
}

fn normalize_generated_files(dir: &Path) -> Result<(), String> {
    for path in walk_ts_files(dir) {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let normalized = normalize_generated_content(&content);
        fs::write(&path, normalized).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn normalize_generated_content(content: &str) -> String {
    let lf = normalize_lf(content);
    let had_final_newline = lf.ends_with('\n');
    let mut normalized = lf
        .lines()
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n");
    if had_final_newline {
        normalized.push('\n');
    }
    normalized
}

fn write_barrel(dir: &Path, types: &[ContractType]) -> std::io::Result<()> {
    let mut names: Vec<&str> = types.iter().map(|ty| ty.name).collect();
    names.sort_unstable();
    names.dedup();
    let mut content = String::from(
        "// Generated barrel. Do not edit by hand; regenerate with\n// `pnpm contracts:export`.\n",
    );
    for name in &names {
        content.push_str(&format!("export * from \"./{name}\";\n"));
    }
    // Normalize to LF so the drift check is OS-independent.
    let content = content.replace("\r\n", "\n");
    fs::write(dir.join("index.ts"), content)
}

/// Compare two directories' `.ts` files; returns the relative paths that differ.
/// Non-TypeScript markers (e.g. `.gitkeep`) are ignored.
fn diff_dirs(expected: &Path, actual: &Path) -> Vec<PathBuf> {
    let mut diffs = Vec::new();
    let expected_files = walk_ts_files(expected);
    let actual_files = walk_ts_files(actual);

    for entry in &actual_files {
        let Ok(rel) = entry.strip_prefix(actual) else {
            continue;
        };
        let expected_path = expected.join(rel);
        match fs::read_to_string(&expected_path) {
            Ok(expected_content) => {
                let Ok(actual_content) = fs::read_to_string(entry) else {
                    diffs.push(rel.to_path_buf());
                    continue;
                };
                // Compare LF-normalized so CRLF on Windows is not drift.
                if normalize_lf(&expected_content) != normalize_lf(&actual_content) {
                    diffs.push(rel.to_path_buf());
                }
            }
            Err(_) => diffs.push(rel.to_path_buf()),
        }
    }
    for entry in expected_files {
        let Ok(rel) = entry.strip_prefix(expected) else {
            continue;
        };
        let actual_path = actual.join(rel);
        if !actual_path.exists() {
            diffs.push(rel.to_path_buf());
        }
    }
    diffs
}

fn normalize_lf(s: &str) -> String {
    s.replace("\r\n", "\n")
}

fn walk_ts_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_ts_files(root, &mut out);
    out
}

fn collect_ts_files(current: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_ts_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "ts") {
            out.push(path);
        }
    }
}

fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    let check_mode = args.iter().any(|a| a == "--check");

    let committed = match generated_dir() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("contracts: failed to resolve generated dir: {err}");
            std::process::exit(1);
        }
    };

    if check_mode {
        let tmp =
            std::env::temp_dir().join(format!("openconkit-contracts-check-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        if let Err(err) = fs::create_dir_all(&tmp) {
            eprintln!("contracts:check: failed to create temp dir: {err}");
            std::process::exit(1);
        }
        if let Err(err) = run_export(&tmp) {
            eprintln!("contracts:check: export failed: {err}");
            let _ = fs::remove_dir_all(&tmp);
            std::process::exit(1);
        }
        let diffs = diff_dirs(&committed, &tmp);
        let _ = fs::remove_dir_all(&tmp);
        if diffs.is_empty() {
            println!("contracts:check: OK (no drift)");
        } else {
            eprintln!(
                "contracts:check: drift detected in {} file(s):",
                diffs.len()
            );
            for diff in &diffs {
                eprintln!("  - {}", diff.display());
            }
            eprintln!("Re-run `pnpm contracts:export` and commit the result.");
            std::process::exit(1);
        }
    } else if let Err(err) = run_export(&committed) {
        eprintln!("contracts:export: failed: {err}");
        std::process::exit(1);
    } else {
        println!(
            "contracts:export: wrote bindings to {}",
            committed.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_generated_content;

    #[test]
    fn generated_content_uses_lf_and_has_no_trailing_whitespace() {
        let source = "export type Example = { \r\n\tvalue: string,\t\r\n};\r\n";

        assert_eq!(
            normalize_generated_content(source),
            "export type Example = {\n\tvalue: string,\n};\n"
        );
    }

    #[test]
    fn generated_content_preserves_missing_final_newline() {
        assert_eq!(
            normalize_generated_content("export type Example = string; "),
            "export type Example = string;"
        );
    }
}
