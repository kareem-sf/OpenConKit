use std::collections::BTreeMap;
use std::path::Path;

use rust_xlsxwriter::{Formula, Workbook};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct FixtureSpec {
    pub id: String,
    pub description: String,
    pub sheets: Vec<SheetSpec>,
    pub expected: ExpectedSpec,
}

#[derive(Debug, Deserialize)]
pub struct SheetSpec {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<CellSpec>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CellSpec {
    Formula { formula: String, result: String },
    Text(String),
    Number(f64),
    Empty(()),
}

#[derive(Debug, Deserialize)]
pub struct ExpectedSpec {
    pub item_rows: usize,
    pub finding_rules: BTreeMap<String, usize>,
    pub literal_cells: Vec<LiteralCellSpec>,
}

#[derive(Debug, Deserialize)]
pub struct LiteralCellSpec {
    pub sheet: String,
    pub cell: String,
    pub raw: String,
}

pub fn parse_spec(raw: &str) -> Result<FixtureSpec, serde_json::Error> {
    serde_json::from_str(raw)
}

pub fn write_workbook(
    spec: &FixtureSpec,
    destination: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut workbook = Workbook::new();
    for sheet_spec in &spec.sheets {
        let sheet = workbook.add_worksheet();
        sheet.set_name(&sheet_spec.name)?;
        for (column, header) in sheet_spec.headers.iter().enumerate() {
            sheet.write_string(0, u16::try_from(column)?, header)?;
        }
        for (row_index, row) in sheet_spec.rows.iter().enumerate() {
            for (column_index, cell) in row.iter().enumerate() {
                let row = u32::try_from(row_index + 1)?;
                let column = u16::try_from(column_index)?;
                match cell {
                    CellSpec::Formula { formula, result } => {
                        sheet.write_formula(
                            row,
                            column,
                            Formula::new(formula).set_result(result),
                        )?;
                    }
                    CellSpec::Text(value) => {
                        sheet.write_string(row, column, value)?;
                    }
                    CellSpec::Number(value) => {
                        sheet.write_number(row, column, *value)?;
                    }
                    CellSpec::Empty(()) => {}
                }
            }
        }
    }
    workbook.save(destination)?;
    Ok(())
}
