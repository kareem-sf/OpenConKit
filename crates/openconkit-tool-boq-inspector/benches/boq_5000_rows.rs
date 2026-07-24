#![forbid(unsafe_code)]
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use openconkit_tool_boq_inspector::BoqInspectorTool;
use openconkit_tool_sdk::{CancellationToken, Tool, ToolRunContext};
use rust_xlsxwriter::Workbook;
use serde_json::json;

const ROW_COUNT: usize = 5_000;

fn write_workload() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("openconkit-benchmark-{nanos}.xlsx"));
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("BOQ").expect("sheet");
    for (column, header) in ["Item", "Description", "Unit", "Quantity", "Rate", "Amount"]
        .into_iter()
        .enumerate()
    {
        sheet
            .write_string(0, u16::try_from(column).expect("column"), header)
            .expect("header");
    }
    for row_index in 0..ROW_COUNT {
        let row = u32::try_from(row_index + 1).expect("row");
        let one_based = row_index + 1;
        let quantity = f64::from(u32::try_from(one_based % 97 + 1).expect("quantity"));
        let rate = f64::from(u32::try_from(one_based % 41 + 1).expect("rate"));
        sheet
            .write_string(row, 0, format!("ITEM-{one_based:05}"))
            .expect("item");
        sheet
            .write_string(
                row,
                1,
                format!("Synthetic measured work item {one_based:05}"),
            )
            .expect("description");
        sheet.write_string(row, 2, "m2").expect("unit");
        sheet.write_number(row, 3, quantity).expect("quantity");
        sheet.write_number(row, 4, rate).expect("rate");
        sheet.write_number(row, 5, quantity * rate).expect("amount");
    }
    workbook.save(&path).expect("workbook");
    path
}

fn benchmark_full_pipeline(criterion: &mut Criterion) {
    let path = write_workload();
    let context = ToolRunContext {
        run_id: "00000000-0000-4000-8000-000000000002".to_string(),
        project_id: "benchmark-project".to_string(),
        source_revision_id: "00000000-0000-4000-8000-000000000001".to_string(),
        workbook_path: path.clone(),
        app_version: "0.0.1".to_string(),
    };
    let input = json!({
        "source_revision_id": context.source_revision_id,
        "rules": []
    });
    let settings = json!({"locale": "en"});
    let tool = BoqInspectorTool::new();
    let engine = tool.engine();
    let mut group = criterion.benchmark_group("boq_inspector");
    group.throughput(Throughput::Elements(
        u64::try_from(ROW_COUNT).expect("row count"),
    ));
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(8));
    group.bench_with_input(
        BenchmarkId::new("xlsx_full_pipeline", ROW_COUNT),
        &ROW_COUNT,
        |bencher, _| {
            bencher.iter(|| {
                black_box(
                    engine
                        .run(
                            &context,
                            &input,
                            &settings,
                            &|_| {},
                            &CancellationToken::new(),
                        )
                        .expect("benchmark run"),
                );
            });
        },
    );
    group.finish();
    std::fs::remove_file(path).expect("cleanup");
}

criterion_group!(benches, benchmark_full_pipeline);
criterion_main!(benches);
