#![forbid(unsafe_code)]

#[path = "../tests/support/mod.rs"]
mod support;

use std::fs;
use std::path::PathBuf;

use support::{parse_spec, write_workbook};

const SPECS: &[(&str, &str)] = &[
    (
        "amount-mismatch",
        include_str!("../../../fixtures/source-specs/amount-mismatch.json"),
    ),
    (
        "formula-and-literal-injection",
        include_str!("../../../fixtures/source-specs/formula-and-literal-injection.json"),
    ),
    (
        "cross-sheet-inconsistency",
        include_str!("../../../fixtures/source-specs/cross-sheet-inconsistency.json"),
    ),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("generated");
    fs::create_dir_all(&output)?;
    for (name, raw) in SPECS {
        let spec = parse_spec(raw)?;
        if spec.id != *name
            || spec.description.trim().is_empty()
            || spec.expected.item_rows == 0
            || spec.expected.finding_rules.is_empty()
            || spec.expected.literal_cells.iter().any(|literal| {
                literal.sheet.trim().is_empty()
                    || literal.cell.trim().is_empty()
                    || literal.raw.is_empty()
            })
        {
            return Err(
                std::io::Error::other(format!("invalid fixture metadata for {name}")).into(),
            );
        }
        let path = output.join(format!("{name}.xlsx"));
        write_workbook(&spec, &path)?;
        println!(
            "{}: {} rows, {} finding rules -> {}",
            spec.id,
            spec.expected.item_rows,
            spec.expected.finding_rules.len(),
            path.display()
        );
    }
    Ok(())
}
