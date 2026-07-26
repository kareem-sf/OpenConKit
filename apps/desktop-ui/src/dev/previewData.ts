import type {
  AnalysisRun,
  AppSettings,
  BoqInspectorOutput,
  BootstrapStatus,
  Finding,
  RunDetails,
  RunHistoryEntry,
  SourceRevision,
  ToolManifest,
} from "@openconkit/contracts";

const STORAGE_GROUP_ID = "quick-analyses";
const SOURCE_ID = "00000000-0000-4000-8000-000000000101";
const RUN_ID = "00000000-0000-4000-8000-000000000201";
const RULE_SET = "2026.07.2";
const STARTED = "2026-07-23T14:42:00Z";
const FINISHED = "2026-07-23T14:42:07Z";

const source: SourceRevision = {
  id: SOURCE_ID,
  project_id: STORAGE_GROUP_ID,
  sha256: "9f8e4b2d3c6a7f1e0d9b8c7a6f5e4d3c2b1a0f099f8e4b2d3c6a7f1e0d9b8c7a",
  original_filename: "Priced BOQ Rev 03.xlsx",
  original_path: null,
  stored_path: `projects/${STORAGE_GROUP_ID}/sources/revision/priced-boq-rev-03.xlsx`,
  size_bytes: 2_486_272,
  imported_at: "2026-07-23T14:40:00Z",
  tool_id: "boq-inspector",
  workbook_metadata: null,
};

const run: AnalysisRun = {
  id: RUN_ID,
  project_id: STORAGE_GROUP_ID,
  source_revision_id: SOURCE_ID,
  tool_id: "boq-inspector",
  tool_version: "0.0.1",
  rule_set_version: RULE_SET,
  app_version: "0.0.1",
  status: "completed",
  started_at: STARTED,
  finished_at: FINISHED,
  structure_diagnostics: {
    rule_set_version: RULE_SET,
    sheets: [
      {
        index: 0,
        name: "BOQ",
        visibility: "visible",
        used_rows: 1312,
        used_columns: 9,
        non_empty_cells: 8214,
        detected_tables: 1,
      },
      {
        index: 1,
        name: "Preliminaries",
        visibility: "visible",
        used_rows: 84,
        used_columns: 7,
        non_empty_cells: 392,
        detected_tables: 1,
      },
    ],
    tables: [
      {
        sheet: "BOQ",
        header_row: 5,
        start_row: 6,
        end_row: 1291,
        columns: [
          {
            column_index: 0,
            column_letter: "A",
            role: "item_number",
            confidence: 0.99,
          },
          {
            column_index: 1,
            column_letter: "B",
            role: "description",
            confidence: 0.98,
          },
          {
            column_index: 2,
            column_letter: "C",
            role: "unit",
            confidence: 0.95,
          },
          {
            column_index: 3,
            column_letter: "D",
            role: "quantity",
            confidence: 0.97,
          },
          {
            column_index: 4,
            column_letter: "E",
            role: "unit_price",
            confidence: 0.96,
          },
          {
            column_index: 5,
            column_letter: "F",
            role: "total_price",
            confidence: 0.98,
          },
        ],
        rows: [],
        interpretation_confidence: 0.94,
        evidence: ["bilingual_header_aliases"],
      },
    ],
    interpretation_confidence: 0.94,
    warnings: [],
  },
  overall_confidence: 0.94,
};

const findingInputs: ReadonlyArray<{
  id: string;
  rule: string;
  severity: Finding["severity"];
  category: Finding["category"];
  title: string;
  explanation: string;
  action: string;
  cell: string;
  value: string;
  formula: string | null;
  confidence: number;
  params?: Record<string, string>;
}> = [
  {
    id: "00000000-0000-4000-8000-000000000301",
    rule: "boq.amount_mismatch",
    severity: "high",
    category: "arithmetic",
    title: "findings.amountMismatch.title",
    explanation: "findings.amountMismatch.explanation",
    action: "findings.amountMismatch.action",
    cell: "F42",
    value: "12500.00",
    formula: "=D42*E42",
    confidence: 0.98,
    params: { expected: "12540.00", actual: "12500.00", difference: "40.00" },
  },
  {
    id: "00000000-0000-4000-8000-000000000302",
    rule: "boq.exact_duplicate",
    severity: "high",
    category: "duplication",
    title: "findings.exactDuplicate.title",
    explanation: "findings.exactDuplicate.explanation",
    action: "findings.exactDuplicate.action",
    cell: "B117",
    value: "Reinforced concrete wall",
    formula: null,
    confidence: 0.92,
  },
  {
    id: "00000000-0000-4000-8000-000000000303",
    rule: "boq.inconsistent_unit",
    severity: "medium",
    category: "inconsistency",
    title: "findings.inconsistentUnit.title",
    explanation: "findings.inconsistentUnit.explanation",
    action: "findings.inconsistentUnit.action",
    cell: "C58",
    value: "m²",
    formula: null,
    confidence: 0.86,
    params: { similarity: "93" },
  },
  {
    id: "00000000-0000-4000-8000-000000000304",
    rule: "boq.value_outlier",
    severity: "medium",
    category: "other",
    title: "findings.valueOutlier.title",
    explanation: "findings.valueOutlier.explanation",
    action: "findings.valueOutlier.action",
    cell: "E210",
    value: "145.60",
    formula: null,
    confidence: 0.78,
    params: { field: "rate", value: "145.60", median: "98.00", peerCount: "18" },
  },
  {
    id: "00000000-0000-4000-8000-000000000305",
    rule: "boq.missing_value",
    severity: "medium",
    category: "omission",
    title: "findings.missingValue.title",
    explanation: "findings.missingValue.explanation",
    action: "findings.missingValue.action",
    cell: "E305",
    value: "",
    formula: null,
    confidence: 0.99,
    params: { field: "rate" },
  },
  {
    id: "00000000-0000-4000-8000-000000000306",
    rule: "boq.formula_unverifiable",
    severity: "low",
    category: "compliance",
    title: "findings.formulaUnverifiable.title",
    explanation: "findings.formulaUnverifiable.explanation",
    action: "findings.formulaUnverifiable.action",
    cell: "F512",
    value: "9400.02",
    formula: "=ROUND(SUBTOTAL(9,F480:F511),2)",
    confidence: 0.68,
  },
];

const findings: Finding[] = findingInputs.map((item) => ({
  id: item.id,
  project_id: STORAGE_GROUP_ID,
  source_revision_id: SOURCE_ID,
  run_id: RUN_ID,
  rule_id: item.rule,
  rule_set_version: RULE_SET,
  category: item.category,
  severity: item.severity,
  confidence: item.confidence,
  title_key: item.title,
  title_params: item.params ?? {},
  explanation_key: item.explanation,
  explanation_params: item.params ?? {},
  suggested_action_key: item.action,
  suggested_action_params: item.params ?? {},
  sheet: "BOQ",
  cell: item.cell,
  range: null,
  source_row_id: `BOQ:${item.cell.replace(/[A-Z]/g, "")}`,
  original_value: item.value,
  original_formula: item.formula,
  evidence: [
    {
      sheet: "BOQ",
      cell: item.cell,
      range: null,
      description_key: "evidence.numericValue",
      snippet: item.value,
    },
  ],
  origin: "deterministic",
  created_at: FINISHED,
}));

const output: BoqInspectorOutput = {
  findings,
  diagnostics: run.structure_diagnostics!,
  summary: {
    item_rows: 1286,
    finding_count: findings.length,
    pareto: [
      {
        context: "BOQ:6-1291",
        currency: "GBP",
        total_amount: "17624000.00",
        top_item_count: 18,
        total_item_count: 214,
        cumulative_share_percent: "80.4",
      },
    ],
  },
  normalized_rows: [],
};

export interface PreviewData {
  bootstrap: BootstrapStatus;
  settings: AppSettings;
  manifests: ToolManifest[];
  revisions: SourceRevision[];
  runs: AnalysisRun[];
  history: RunHistoryEntry[];
  runDetails: RunDetails;
}

export function previewData(): PreviewData {
  return {
    bootstrap: {
      home_path: "C:\\Users\\demo\\.openconkit",
      created_fresh: false,
      structure_validated: true,
      recovered_from_interrupt: false,
      config_warnings: [],
      database_migrations: [],
      backups_created: [],
    },
    settings: {
      schema_version: 2,
      onboarding_completed: true,
      language: "system",
      theme: "system",
      update_channel: "stable",
      tolerances: {
        absolute_tolerance: "0.01",
        relative_tolerance: "0.001",
        decimal_precision: 2,
      },
      privacy: {
        ai_features_enabled: false,
        diagnostic_logging_enabled: false,
      },
      advanced: {
        use_system_codex: false,
        system_codex_binary: null,
      },
      last_successful_update_check: null,
    },
    manifests: [
      {
        id: "boq-inspector",
        contract_version: 2,
        tool_version: "0.0.1",
        name_key: "tools.boqInspector.name",
        description_key: "tools.boqInspector.description",
        icon: "tools/boq-inspector.svg",
        route: "/tools/boq-inspector",
      },
    ],
    revisions: [source],
    runs: [run],
    history: [
      {
        run,
        source_sha256: source.sha256,
        finding_count: findings.length,
        export_count: 0,
        ai_analysis_count: 0,
        latest_ai_status: null,
      },
    ],
    runDetails: {
      run,
      findings,
      output,
      exports: [],
      ai_analyses: [],
    },
  };
}
