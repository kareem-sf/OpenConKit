//! Bounded read-only XLS/XLSX ingestion.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::Path;

use calamine::{open_workbook, Data, Dimensions, Reader, SheetType, SheetVisible, Xls, Xlsx};
use zip::ZipArchive;

use crate::error::SpreadsheetError;
use crate::limits::WorkbookLimits;
use crate::model::{
    CellRegion, DateSystem, IngestedCell, IngestedSheet, IngestedWorkbook, NormalizedCellValue,
    SheetKind, SheetVisibility, WorkbookFormat,
};
use crate::observer::{IngestionObserver, IngestionProgress, IngestionStage};

const CANCELLATION_INTERVAL: usize = 256;

/// Ingest a workbook with explicit limits and caller-provided observation.
pub fn ingest_with_observer(
    path: &Path,
    limits: &WorkbookLimits,
    observer: &dyn IngestionObserver,
) -> Result<IngestedWorkbook, SpreadsheetError> {
    if let Some(field) = limits.first_invalid_field() {
        return Err(SpreadsheetError::InvalidLimit { field });
    }
    notify(observer, IngestionStage::FileValidation, None, None, 0)?;

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or(SpreadsheetError::UnsupportedExtension)?;
    if extension != "xls" && extension != "xlsx" {
        return Err(SpreadsheetError::UnsupportedExtension);
    }
    let metadata = fs::metadata(path).map_err(io_error)?;
    if !metadata.is_file() {
        return Err(SpreadsheetError::NotRegularFile);
    }
    if metadata.len() > limits.max_file_size_bytes {
        return Err(SpreadsheetError::FileTooLarge {
            actual_bytes: metadata.len(),
            max_bytes: limits.max_file_size_bytes,
        });
    }

    match extension.as_str() {
        "xlsx" => {
            notify(observer, IngestionStage::ArchiveValidation, None, None, 0)?;
            preflight_xlsx_archive(path, limits)?;
            read_xlsx(path, limits, observer)
        }
        "xls" => read_xls(path, limits, observer),
        _ => Err(SpreadsheetError::UnsupportedExtension),
    }
}

fn preflight_xlsx_archive(path: &Path, limits: &WorkbookLimits) -> Result<(), SpreadsheetError> {
    let file = File::open(path).map_err(io_error)?;
    let mut archive = ZipArchive::new(file).map_err(archive_error)?;
    if archive.len() > limits.max_archive_entries {
        return Err(SpreadsheetError::TooManyArchiveEntries {
            actual: archive.len(),
            max: limits.max_archive_entries,
        });
    }

    let mut total_uncompressed = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(archive_error)?;
        let name = safe_excerpt(entry.name());
        if entry.enclosed_name().is_none() || entry.is_symlink() {
            return Err(SpreadsheetError::UnsafeArchiveEntry { entry: name });
        }
        if entry.encrypted() {
            return Err(SpreadsheetError::EncryptedArchiveEntry { entry: name });
        }
        let size = entry.size();
        if size > limits.max_archive_entry_uncompressed_bytes {
            return Err(SpreadsheetError::ArchiveEntryTooLarge {
                entry: name,
                actual_bytes: size,
                max_bytes: limits.max_archive_entry_uncompressed_bytes,
            });
        }
        total_uncompressed =
            total_uncompressed
                .checked_add(size)
                .ok_or(SpreadsheetError::ArchiveTooLarge {
                    actual_bytes: u64::MAX,
                    max_bytes: limits.max_archive_uncompressed_bytes,
                })?;
        if total_uncompressed > limits.max_archive_uncompressed_bytes {
            return Err(SpreadsheetError::ArchiveTooLarge {
                actual_bytes: total_uncompressed,
                max_bytes: limits.max_archive_uncompressed_bytes,
            });
        }
        if size > 0 {
            let allowed = entry
                .compressed_size()
                .saturating_mul(limits.max_compression_ratio);
            if entry.compressed_size() == 0 || size > allowed {
                return Err(SpreadsheetError::SuspiciousCompressionRatio {
                    entry: name,
                    max_ratio: limits.max_compression_ratio,
                });
            }
        }
    }
    Ok(())
}

fn read_xlsx(
    path: &Path,
    limits: &WorkbookLimits,
    observer: &dyn IngestionObserver,
) -> Result<IngestedWorkbook, SpreadsheetError> {
    let mut workbook: Xlsx<_> = open_workbook(path).map_err(parse_error)?;
    let metadata = workbook.sheets_metadata().to_vec();
    validate_sheet_count(metadata.len(), limits)?;
    let sheet_count = metadata.len();
    notify(
        observer,
        IngestionStage::WorkbookMetadata,
        None,
        Some(sheet_count),
        0,
    )?;

    let date_system = if workbook.has_1904_epoch() {
        DateSystem::Excel1904
    } else {
        DateSystem::Excel1900
    };
    let mut accumulator = Accumulator::new(limits);
    let mut sheets = Vec::with_capacity(sheet_count);

    for (index, sheet) in metadata.into_iter().enumerate() {
        notify(
            observer,
            IngestionStage::Worksheet,
            Some(index),
            Some(sheet_count),
            accumulator.total_cells,
        )?;
        let mut model = sheet_shell(index, &sheet.name, sheet.typ, sheet.visible)?;
        if sheet.typ == SheetType::WorkSheet {
            let merged = workbook
                .merge_cells_by_sheet_name(&sheet.name)
                .map_err(parse_error)?;
            model.merged_regions = validate_merged(&sheet.name, merged, limits)?;

            let mut reader = workbook
                .worksheet_cells_reader(&sheet.name)
                .map_err(parse_error)?;
            let declared = dimensions_to_region(reader.dimensions());
            validate_region(&sheet.name, &declared, limits)?;
            model.declared_range = Some(declared);

            let mut records_seen = 0usize;
            while let Some(record) = reader.next_cell_with_formula().map_err(parse_error)? {
                records_seen = records_seen.saturating_add(1);
                if records_seen % CANCELLATION_INTERVAL == 0 {
                    notify(
                        observer,
                        IngestionStage::Worksheet,
                        Some(index),
                        Some(sheet_count),
                        accumulator.total_cells,
                    )?;
                }
                let data: Data = record.value.into();
                if matches!(data, Data::Empty) && record.formula.is_none() {
                    continue;
                }
                let cell = accumulator.retain(&sheet.name, record.pos, data, record.formula)?;
                extend_used_range(&mut model.used_range, cell.row, cell.column);
                model.cells.push(cell);
            }
        }
        sheets.push(model);
    }

    notify(
        observer,
        IngestionStage::Complete,
        None,
        Some(sheets.len()),
        accumulator.total_cells,
    )?;
    Ok(IngestedWorkbook {
        format: WorkbookFormat::Xlsx,
        date_system,
        sheets,
        total_cells: accumulator.total_cells,
        total_text_bytes: accumulator.total_text_bytes,
    })
}

fn read_xls(
    path: &Path,
    limits: &WorkbookLimits,
    observer: &dyn IngestionObserver,
) -> Result<IngestedWorkbook, SpreadsheetError> {
    let mut workbook: Xls<_> = open_workbook(path).map_err(parse_error)?;
    let metadata = workbook.sheets_metadata().to_vec();
    validate_sheet_count(metadata.len(), limits)?;
    let sheet_count = metadata.len();
    notify(
        observer,
        IngestionStage::WorkbookMetadata,
        None,
        Some(sheet_count),
        0,
    )?;

    let date_system = if workbook.has_1904_epoch() {
        DateSystem::Excel1904
    } else {
        DateSystem::Excel1900
    };
    let mut accumulator = Accumulator::new(limits);
    let mut sheets = Vec::with_capacity(sheet_count);

    for (index, sheet) in metadata.into_iter().enumerate() {
        notify(
            observer,
            IngestionStage::Worksheet,
            Some(index),
            Some(sheet_count),
            accumulator.total_cells,
        )?;
        let mut model = sheet_shell(index, &sheet.name, sheet.typ, sheet.visible)?;
        if sheet.typ == SheetType::WorkSheet {
            let merged = workbook
                .merge_cells_by_sheet_name(&sheet.name)
                .map_err(parse_error)?;
            model.merged_regions = validate_merged(&sheet.name, merged, limits)?;

            // Calamine's legacy XLS API materializes a range. Validate the
            // resulting dimensions before retaining any cells; outer file and
            // retained-cell bounds still apply independently.
            let range = workbook.worksheet_range(&sheet.name).map_err(parse_error)?;
            if let (Some(start), Some(end)) = (range.start(), range.end()) {
                let declared = CellRegion {
                    start_row: start.0,
                    start_column: start.1,
                    end_row: end.0,
                    end_column: end.1,
                };
                validate_region(&sheet.name, &declared, limits)?;
                model.declared_range = Some(declared);
            }
            let formula_range = workbook
                .worksheet_formula(&sheet.name)
                .map_err(parse_error)?;
            let mut formulas = collect_formulas(&sheet.name, &formula_range, limits)?;

            let range_start = range.start().unwrap_or((0, 0));
            for (relative_row, relative_column, data) in range.used_cells() {
                let row = add_coordinate(range_start.0, relative_row)?;
                let column = add_coordinate(range_start.1, relative_column)?;
                let formula = formulas.remove(&(row, column));
                let cell = accumulator.retain(&sheet.name, (row, column), data.clone(), formula)?;
                extend_used_range(&mut model.used_range, row, column);
                model.cells.push(cell);
                if accumulator.total_cells % CANCELLATION_INTERVAL == 0 {
                    notify(
                        observer,
                        IngestionStage::Worksheet,
                        Some(index),
                        Some(sheet_count),
                        accumulator.total_cells,
                    )?;
                }
            }
            // Preserve formula cells whose cached value is missing.
            for ((row, column), formula) in formulas {
                let cell =
                    accumulator.retain(&sheet.name, (row, column), Data::Empty, Some(formula))?;
                extend_used_range(&mut model.used_range, row, column);
                model.cells.push(cell);
            }
            model
                .cells
                .sort_unstable_by_key(|cell| (cell.row, cell.column));
        }
        sheets.push(model);
    }

    notify(
        observer,
        IngestionStage::Complete,
        None,
        Some(sheets.len()),
        accumulator.total_cells,
    )?;
    Ok(IngestedWorkbook {
        format: WorkbookFormat::Xls,
        date_system,
        sheets,
        total_cells: accumulator.total_cells,
        total_text_bytes: accumulator.total_text_bytes,
    })
}

fn collect_formulas(
    sheet: &str,
    formulas: &calamine::Range<String>,
    limits: &WorkbookLimits,
) -> Result<BTreeMap<(u32, u32), String>, SpreadsheetError> {
    let mut output = BTreeMap::new();
    let start = formulas.start().unwrap_or((0, 0));
    for (relative_row, relative_column, formula) in formulas.used_cells() {
        let row = add_coordinate(start.0, relative_row)?;
        let column = add_coordinate(start.1, relative_column)?;
        validate_position(sheet, row, column, limits)?;
        if !formula.is_empty() {
            let address = cell_address(row, column);
            if formula.len() > limits.max_formula_bytes {
                return Err(SpreadsheetError::FormulaTooLarge {
                    cell: format!("{sheet}!{address}"),
                    actual_bytes: formula.len(),
                    max_bytes: limits.max_formula_bytes,
                });
            }
            if output.len() >= limits.max_cells {
                return Err(SpreadsheetError::TooManyCells {
                    max: limits.max_cells,
                });
            }
            output.insert((row, column), formula.clone());
        }
    }
    Ok(output)
}

fn add_coordinate(base: u32, relative: usize) -> Result<u32, SpreadsheetError> {
    let relative = u32::try_from(relative).map_err(|_| SpreadsheetError::Parse {
        message: "worksheet coordinate exceeds u32".to_string(),
    })?;
    base.checked_add(relative)
        .ok_or_else(|| SpreadsheetError::Parse {
            message: "worksheet coordinate overflow".to_string(),
        })
}

fn sheet_shell(
    index: usize,
    name: &str,
    kind: SheetType,
    visibility: SheetVisible,
) -> Result<IngestedSheet, SpreadsheetError> {
    let index = u32::try_from(index).map_err(|_| SpreadsheetError::Parse {
        message: "sheet index exceeds u32".to_string(),
    })?;
    Ok(IngestedSheet {
        index,
        name: name.to_string(),
        kind: convert_sheet_kind(kind),
        visibility: convert_visibility(visibility),
        declared_range: None,
        used_range: None,
        merged_regions: vec![],
        // Calamine 0.36 exposes sheet visibility but not row/column hidden
        // state through its stable reader API. Preserve that uncertainty.
        hidden_rows: None,
        hidden_columns: None,
        cells: vec![],
    })
}

fn validate_sheet_count(actual: usize, limits: &WorkbookLimits) -> Result<(), SpreadsheetError> {
    if actual > limits.max_sheets {
        Err(SpreadsheetError::TooManySheets {
            actual,
            max: limits.max_sheets,
        })
    } else {
        Ok(())
    }
}

fn validate_merged(
    sheet: &str,
    regions: Vec<Dimensions>,
    limits: &WorkbookLimits,
) -> Result<Vec<CellRegion>, SpreadsheetError> {
    if regions.len() > limits.max_merged_regions_per_sheet {
        return Err(SpreadsheetError::TooManyMergedRegions {
            sheet: sheet.to_string(),
            actual: regions.len(),
            max: limits.max_merged_regions_per_sheet,
        });
    }
    regions
        .into_iter()
        .map(|dimensions| {
            let region = dimensions_to_region(dimensions);
            validate_region(sheet, &region, limits)?;
            Ok(region)
        })
        .collect()
}

fn dimensions_to_region(dimensions: Dimensions) -> CellRegion {
    CellRegion {
        start_row: dimensions.start.0,
        start_column: dimensions.start.1,
        end_row: dimensions.end.0,
        end_column: dimensions.end.1,
    }
}

fn validate_region(
    sheet: &str,
    region: &CellRegion,
    limits: &WorkbookLimits,
) -> Result<(), SpreadsheetError> {
    validate_position(sheet, region.end_row, region.end_column, limits)
}

fn validate_position(
    sheet: &str,
    row: u32,
    column: u32,
    limits: &WorkbookLimits,
) -> Result<(), SpreadsheetError> {
    let one_based_row = row.saturating_add(1);
    let one_based_column = column.saturating_add(1);
    if one_based_row > limits.max_rows_per_sheet {
        return Err(SpreadsheetError::TooManyRows {
            sheet: sheet.to_string(),
            actual: one_based_row,
            max: limits.max_rows_per_sheet,
        });
    }
    if one_based_column > limits.max_columns_per_sheet {
        return Err(SpreadsheetError::TooManyColumns {
            sheet: sheet.to_string(),
            actual: one_based_column,
            max: limits.max_columns_per_sheet,
        });
    }
    Ok(())
}

fn extend_used_range(range: &mut Option<CellRegion>, row: u32, column: u32) {
    match range {
        Some(region) => {
            region.start_row = region.start_row.min(row);
            region.start_column = region.start_column.min(column);
            region.end_row = region.end_row.max(row);
            region.end_column = region.end_column.max(column);
        }
        None => {
            *range = Some(CellRegion {
                start_row: row,
                start_column: column,
                end_row: row,
                end_column: column,
            });
        }
    }
}

struct Accumulator<'a> {
    limits: &'a WorkbookLimits,
    total_cells: usize,
    total_text_bytes: usize,
}

impl<'a> Accumulator<'a> {
    fn new(limits: &'a WorkbookLimits) -> Self {
        Self {
            limits,
            total_cells: 0,
            total_text_bytes: 0,
        }
    }

    fn retain(
        &mut self,
        sheet: &str,
        position: (u32, u32),
        data: Data,
        formula: Option<String>,
    ) -> Result<IngestedCell, SpreadsheetError> {
        validate_position(sheet, position.0, position.1, self.limits)?;
        let address = cell_address(position.0, position.1);
        let (raw_value, normalized_value) = normalize_data(data);
        if raw_value.len() > self.limits.max_cell_text_bytes {
            return Err(SpreadsheetError::CellTextTooLarge {
                cell: format!("{sheet}!{address}"),
                actual_bytes: raw_value.len(),
                max_bytes: self.limits.max_cell_text_bytes,
            });
        }
        if let Some(formula) = &formula {
            if formula.len() > self.limits.max_formula_bytes {
                return Err(SpreadsheetError::FormulaTooLarge {
                    cell: format!("{sheet}!{address}"),
                    actual_bytes: formula.len(),
                    max_bytes: self.limits.max_formula_bytes,
                });
            }
        }
        self.total_cells =
            self.total_cells
                .checked_add(1)
                .ok_or(SpreadsheetError::TooManyCells {
                    max: self.limits.max_cells,
                })?;
        if self.total_cells > self.limits.max_cells {
            return Err(SpreadsheetError::TooManyCells {
                max: self.limits.max_cells,
            });
        }
        let retained_bytes = raw_value
            .len()
            .checked_add(formula.as_ref().map_or(0, String::len))
            .ok_or(SpreadsheetError::TotalTextTooLarge {
                max_bytes: self.limits.max_total_text_bytes,
            })?;
        self.total_text_bytes = self.total_text_bytes.checked_add(retained_bytes).ok_or(
            SpreadsheetError::TotalTextTooLarge {
                max_bytes: self.limits.max_total_text_bytes,
            },
        )?;
        if self.total_text_bytes > self.limits.max_total_text_bytes {
            return Err(SpreadsheetError::TotalTextTooLarge {
                max_bytes: self.limits.max_total_text_bytes,
            });
        }
        Ok(IngestedCell {
            row: position.0,
            column: position.1,
            address,
            raw_value,
            normalized_value,
            displayed_value: None,
            formula,
        })
    }
}

fn normalize_data(data: Data) -> (String, NormalizedCellValue) {
    match data {
        Data::Empty => (String::new(), NormalizedCellValue::Empty),
        Data::Int(value) => (value.to_string(), NormalizedCellValue::Integer(value)),
        Data::Float(value) if value.is_finite() => {
            let raw = value.to_string();
            (raw.clone(), NormalizedCellValue::Number(raw))
        }
        Data::Float(value) => {
            let raw = value.to_string();
            (
                raw.clone(),
                NormalizedCellValue::Error(format!("non_finite:{raw}")),
            )
        }
        Data::String(value) => {
            let normalized = value.trim().to_string();
            (value, NormalizedCellValue::Text(normalized))
        }
        Data::Bool(value) => (value.to_string(), NormalizedCellValue::Boolean(value)),
        Data::DateTime(value) if value.is_duration() => {
            let serial = value.as_f64().to_string();
            (
                serial.clone(),
                NormalizedCellValue::ExcelDuration { serial },
            )
        }
        Data::DateTime(value) => {
            let serial = value.as_f64().to_string();
            let (year, month, day, hour, minute, second, millis) = value.to_ymd_hms_milli();
            let rendered = if millis == 0 {
                format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}")
            } else {
                format!(
                    "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}"
                )
            };
            (
                serial.clone(),
                NormalizedCellValue::DateTime { serial, rendered },
            )
        }
        Data::DateTimeIso(value) => (value.clone(), NormalizedCellValue::DateTimeIso(value)),
        Data::DurationIso(value) => (value.clone(), NormalizedCellValue::DurationIso(value)),
        Data::Error(value) => {
            let error = value.to_string();
            (error.clone(), NormalizedCellValue::Error(error))
        }
    }
}

fn cell_address(row: u32, column: u32) -> String {
    let mut value = u64::from(column) + 1;
    let mut letters = Vec::new();
    while value > 0 {
        let remainder = ((value - 1) % 26) as u8;
        letters.push(char::from(b'A' + remainder));
        value = (value - 1) / 26;
    }
    letters.reverse();
    let column: String = letters.into_iter().collect();
    format!("{column}{}", u64::from(row) + 1)
}

fn convert_sheet_kind(kind: SheetType) -> SheetKind {
    match kind {
        SheetType::WorkSheet => SheetKind::Worksheet,
        SheetType::DialogSheet => SheetKind::DialogSheet,
        SheetType::MacroSheet => SheetKind::MacroSheet,
        SheetType::ChartSheet => SheetKind::ChartSheet,
        SheetType::Vba => SheetKind::Vba,
    }
}

fn convert_visibility(visibility: SheetVisible) -> SheetVisibility {
    match visibility {
        SheetVisible::Visible => SheetVisibility::Visible,
        SheetVisible::Hidden => SheetVisibility::Hidden,
        SheetVisible::VeryHidden => SheetVisibility::VeryHidden,
    }
}

fn notify(
    observer: &dyn IngestionObserver,
    stage: IngestionStage,
    sheet_index: Option<usize>,
    sheet_count: Option<usize>,
    cells_read: usize,
) -> Result<(), SpreadsheetError> {
    if observer.is_cancelled() {
        return Err(SpreadsheetError::Cancelled);
    }
    observer.on_progress(&IngestionProgress {
        stage,
        sheet_index,
        sheet_count,
        cells_read,
    });
    Ok(())
}

fn safe_excerpt(raw: &str) -> String {
    raw.chars().take(120).collect()
}

fn io_error(error: std::io::Error) -> SpreadsheetError {
    SpreadsheetError::Io {
        message: error.to_string(),
    }
}

fn parse_error(error: impl std::fmt::Display) -> SpreadsheetError {
    SpreadsheetError::Parse {
        message: error.to_string(),
    }
}

fn archive_error(error: impl std::fmt::Display) -> SpreadsheetError {
    SpreadsheetError::Archive {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use rust_xlsxwriter::{Format, Formula, Workbook};
    use sha2::{Digest, Sha256};

    use super::*;

    fn temp_path(stem: &str, extension: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "openconkit-ingestion-{stem}-{}-{nanos}.{extension}",
            std::process::id()
        ))
    }

    fn write_sample_xlsx(path: &Path) {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("BOQ").expect("sheet name");
        worksheet
            .merge_range(0, 0, 0, 2, "Bill of Quantities", &Format::new())
            .expect("merge");
        worksheet.write_string(2, 0, "Item").expect("header");
        worksheet.write_string(2, 1, "Description").expect("header");
        worksheet.write_string(2, 2, "Amount").expect("header");
        worksheet.write_number(3, 0, 1).expect("item");
        worksheet
            .write_string(3, 1, "Concrete")
            .expect("description");
        worksheet
            .write_formula(3, 2, Formula::new("=2*3").set_result("6"))
            .expect("formula");
        let hidden = workbook.add_worksheet();
        hidden.set_name("Rates").expect("sheet name");
        hidden.set_hidden(true);
        hidden.write_number(0, 0, 123).expect("hidden cell");
        workbook.save(path).expect("save fixture");
    }

    fn sha256(path: &Path) -> [u8; 32] {
        Sha256::digest(std::fs::read(path).expect("read")).into()
    }

    #[test]
    fn xlsx_ingestion_preserves_structure_values_formula_and_visibility() {
        let path = temp_path("sample", "xlsx");
        write_sample_xlsx(&path);

        let model = ingest_with_observer(
            &path,
            &WorkbookLimits::default(),
            &crate::observer::NoopObserver,
        )
        .expect("ingest");

        assert_eq!(model.format, WorkbookFormat::Xlsx);
        assert_eq!(model.sheets.len(), 2);
        assert_eq!(model.sheets[0].name, "BOQ");
        assert_eq!(model.sheets[0].merged_regions.len(), 1);
        assert_eq!(model.sheets[1].visibility, SheetVisibility::Hidden);
        let amount = model.sheets[0]
            .cells
            .iter()
            .find(|cell| cell.address == "C4")
            .expect("formula cell");
        assert_eq!(amount.formula.as_deref(), Some("2*3"));
        assert_eq!(amount.raw_value, "6");
        assert_eq!(
            amount.normalized_value,
            NormalizedCellValue::Number("6".to_string())
        );
        assert!(amount.displayed_value.is_none());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn ingestion_never_modifies_the_source_hash() {
        let path = temp_path("immutable", "xlsx");
        write_sample_xlsx(&path);
        let before = sha256(&path);
        ingest_with_observer(
            &path,
            &WorkbookLimits::default(),
            &crate::observer::NoopObserver,
        )
        .expect("ingest");
        let after = sha256(&path);
        assert_eq!(before, after);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn declared_dimensions_are_rejected_before_cells_are_retained() {
        let path = temp_path("dimensions", "xlsx");
        let mut workbook = Workbook::new();
        workbook
            .add_worksheet()
            .write_string(99, 0, "too far")
            .expect("write");
        workbook.save(&path).expect("save");
        let limits = WorkbookLimits {
            max_rows_per_sheet: 10,
            ..WorkbookLimits::default()
        };
        let error = ingest_with_observer(&path, &limits, &crate::observer::NoopObserver)
            .expect_err("dimension rejected");
        assert!(matches!(error, SpreadsheetError::TooManyRows { .. }));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn archive_ratio_limit_is_enforced_from_central_directory_metadata() {
        let path = temp_path("ratio", "xlsx");
        let mut workbook = Workbook::new();
        workbook
            .add_worksheet()
            .write_string(0, 0, "A".repeat(20_000))
            .expect("write");
        workbook.save(&path).expect("save");
        let limits = WorkbookLimits {
            max_compression_ratio: 1,
            ..WorkbookLimits::default()
        };
        let error = ingest_with_observer(&path, &limits, &crate::observer::NoopObserver)
            .expect_err("ratio rejected");
        assert!(matches!(
            error,
            SpreadsheetError::SuspiciousCompressionRatio { .. }
        ));
        std::fs::remove_file(path).ok();
    }

    struct CancelledObserver {
        cancelled: AtomicBool,
    }

    impl IngestionObserver for CancelledObserver {
        fn is_cancelled(&self) -> bool {
            self.cancelled.load(Ordering::Relaxed)
        }
    }

    #[test]
    fn cancellation_is_checked_before_opening() {
        let observer = CancelledObserver {
            cancelled: AtomicBool::new(true),
        };
        let error = ingest_with_observer(
            Path::new("does-not-need-to-exist.xlsx"),
            &WorkbookLimits::default(),
            &observer,
        )
        .expect_err("cancelled");
        assert!(matches!(error, SpreadsheetError::Cancelled));
    }

    #[test]
    fn unsupported_extension_is_rejected() {
        let path = temp_path("unsupported", "csv");
        std::fs::write(&path, b"a,b").expect("write");
        let error = ingest_with_observer(
            &path,
            &WorkbookLimits::default(),
            &crate::observer::NoopObserver,
        )
        .expect_err("rejected");
        assert!(matches!(error, SpreadsheetError::UnsupportedExtension));
        std::fs::remove_file(path).ok();
    }
}
