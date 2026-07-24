#![forbid(unsafe_code)]

mod support;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use openconkit_tool_boq_inspector::{BoqInspectorOutput, BoqInspectorTool, BoqNormalizedFact};
use openconkit_tool_sdk::{CancellationToken, Tool, ToolRunContext};
use serde_json::json;

use support::{parse_spec, write_workbook};

const SPECS: &[(&str, &str)] = &[
    (
        "amount-mismatch.json",
        include_str!("../../../fixtures/source-specs/amount-mismatch.json"),
    ),
    (
        "formula-and-literal-injection.json",
        include_str!("../../../fixtures/source-specs/formula-and-literal-injection.json"),
    ),
    (
        "cross-sheet-inconsistency.json",
        include_str!("../../../fixtures/source-specs/cross-sheet-inconsistency.json"),
    ),
];

fn temporary_workbook(id: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("openconkit-fixture-{id}-{nanos}.xlsx"))
}

fn run_fixture(path: PathBuf) -> BoqInspectorOutput {
    let context = ToolRunContext {
        run_id: "00000000-0000-4000-8000-000000000002".to_string(),
        project_id: "fixture-project".to_string(),
        source_revision_id: "00000000-0000-4000-8000-000000000001".to_string(),
        workbook_path: path,
        app_version: "0.0.1".to_string(),
    };
    let output = BoqInspectorTool::new()
        .engine()
        .run(
            &context,
            &json!({
                "source_revision_id": context.source_revision_id,
                "rules": []
            }),
            &json!({"locale": "en"}),
            &|_| {},
            &CancellationToken::new(),
        )
        .expect("fixture run");
    serde_json::from_value(output).expect("typed output")
}

fn facts(output: &BoqInspectorOutput) -> Vec<(&str, &BoqNormalizedFact)> {
    let mut facts = Vec::new();
    for row in &output.normalized_rows {
        for fact in [
            row.item_code.as_ref(),
            row.description.as_ref(),
            row.unit.as_ref(),
            row.quantity.as_ref(),
            row.rate_text.as_ref(),
            row.rate.as_ref(),
            row.amount.as_ref(),
            row.currency.as_ref(),
        ]
        .into_iter()
        .flatten()
        .chain(row.error_cells.iter())
        {
            facts.push((row.sheet.as_str(), fact));
        }
    }
    facts
}

#[test]
fn source_specs_generate_reproducible_workbooks_with_exact_findings() {
    for (file_name, raw) in SPECS {
        let spec = parse_spec(raw).expect("valid source spec");
        assert!(!spec.description.trim().is_empty(), "{file_name}");
        let path = temporary_workbook(&spec.id);
        write_workbook(&spec, &path).expect("generate workbook");
        let output = run_fixture(path.clone());
        assert_eq!(
            output.summary.item_rows, spec.expected.item_rows,
            "{file_name}: item row count"
        );

        let mut actual_rules = BTreeMap::new();
        for finding in &output.findings {
            *actual_rules
                .entry(finding.rule_id.clone())
                .or_insert(0_usize) += 1;
        }
        assert_eq!(
            actual_rules, spec.expected.finding_rules,
            "{file_name}: exact planted findings"
        );

        let facts = facts(&output);
        for literal in &spec.expected.literal_cells {
            assert!(
                facts.iter().any(|(sheet, fact)| {
                    *sheet == literal.sheet
                        && fact.cell == literal.cell
                        && fact.raw == literal.raw
                        && fact.formula.is_none()
                }),
                "{file_name}: literal formula-looking text was reinterpreted"
            );
        }
        std::fs::remove_file(path).expect("cleanup fixture");
    }
}
