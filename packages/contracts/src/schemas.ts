import { z } from "zod";

import type {
  AdvancedSettings,
  AiAccountSnapshot,
  AiAnalysis,
  AiLoginChallenge,
  AiRateLimitSnapshot,
  AiRateLimitWindow,
  AiReviewScope,
  AiRuntimeStatus,
  AnalysisRun,
  AnalysisTolerances,
  AppSettings,
  AvailableUpdate,
  BoqAiPrioritizedRisk,
  BoqAiReview,
  BoqInspectorInput,
  BoqInspectorOutput,
  BoqInspectorSettings,
  BoqInspectorSummary,
  BoqNormalizedFact,
  BoqNormalizedRow,
  BootstrapStatus,
  ClassifiedRow,
  ColumnRoleAssignment,
  DetectedTable,
  Evidence,
  ExportRecord,
  Finding,
  ParetoAnalysis,
  PrivacySettings,
  Project,
  ProjectMetadata,
  IpcError,
  RunDetails,
  RunHistoryEntry,
  SettingsPatch,
  SheetInventory,
  SourceRevision,
  ToolManifest,
  ToolNavItem,
  ToolProgress,
  ToolProgressEvent,
  ToolRunResponse,
  UpdateCheckResult,
  UpdateProgressEvent,
  WorkbookDiagnostics,
} from "./generated/index";

const decimalSchema = z
  .string()
  .regex(/^(?:0|[1-9]\d*)(?:\.\d+)?$/, "expected a non-negative decimal string");
const timestampSchema = z.string().min(1);
const nullableTimestampSchema = timestampSchema.nullable();
const uuidSchema = z.uuid();
const confidenceSchema = z.number().min(0).max(1);
const cellRefSchema = z.string().regex(/^[A-Z]{1,3}[1-9]\d{0,6}$/);
const cellRangeSchema = z.strictObject({
  start: cellRefSchema,
  end: cellRefSchema,
});
const safeAssetPathSchema = z
  .string()
  .min(1)
  .refine(
    (path) =>
      !path.startsWith("/") &&
      !path.includes("\\") &&
      !path.includes(":") &&
      path.split("/").every((segment) => segment !== "" && segment !== "." && segment !== ".."),
    "expected a safe relative forward-slashed path",
  );

/** Stable project slug accepted by the Rust domain. */
export const projectIdSchema = z
  .string()
  .min(1)
  .max(64)
  .regex(
    /^[a-z0-9]+(?:-[a-z0-9]+)*$/,
    "expected lowercase ASCII letters, digits, and single internal hyphens",
  );

export const languageSchema = z.enum(["system", "en", "ar"]);
export const themeSchema = z.enum(["system", "light", "dark"]);
export const updateChannelSchema = z.enum(["stable", "beta"]);

export const analysisTolerancesSchema: z.ZodType<AnalysisTolerances> = z.strictObject({
  absolute_tolerance: decimalSchema,
  relative_tolerance: decimalSchema,
  decimal_precision: z.number().int().min(0).max(6),
});

export const privacySettingsSchema: z.ZodType<PrivacySettings> = z.strictObject({
  ai_features_enabled: z.boolean(),
  diagnostic_logging_enabled: z.boolean(),
});

export const advancedSettingsSchema: z.ZodType<AdvancedSettings> = z.strictObject({
  use_system_codex: z.boolean(),
  system_codex_binary: z
    .string()
    .trim()
    .min(1)
    .max(1_024)
    .refine((value) => /(?:^|[/\\])codex(?:\.exe)?$/i.test(value))
    .nullable(),
});

export const appSettingsSchema: z.ZodType<AppSettings> = z.strictObject({
  schema_version: z.literal(2),
  onboarding_completed: z.boolean(),
  language: languageSchema,
  theme: themeSchema,
  update_channel: updateChannelSchema,
  tolerances: analysisTolerancesSchema,
  privacy: privacySettingsSchema,
  advanced: advancedSettingsSchema,
  last_successful_update_check: nullableTimestampSchema,
});

export const settingsPatchSchema: z.ZodType<SettingsPatch> = z.strictObject({
  onboarding_completed: z.boolean().nullable(),
  language: languageSchema.nullable(),
  theme: themeSchema.nullable(),
  update_channel: updateChannelSchema.nullable(),
  tolerances: analysisTolerancesSchema.nullable(),
  privacy: privacySettingsSchema.nullable(),
  advanced: advancedSettingsSchema.nullable(),
  last_successful_update_check: nullableTimestampSchema,
});

const semanticVersionSchema = z
  .string()
  .regex(
    /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/,
    "expected a semantic version",
  );

export const availableUpdateSchema: z.ZodType<AvailableUpdate> = z.strictObject({
  version: semanticVersionSchema,
  notes: z.string().max(16_384).nullable(),
  published_at: timestampSchema.nullable(),
  size_bytes: z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER).nullable(),
  can_install: z.boolean(),
  manual_download_url: z
    .string()
    .regex(
      /^https:\/\/github\.com\/kareem-sf\/OpenConKit\/releases\/tag\/v(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/,
    ),
});

export const updateCheckResultSchema: z.ZodType<UpdateCheckResult> = z.strictObject({
  checked_at: timestampSchema,
  channel: updateChannelSchema,
  current_version: semanticVersionSchema,
  portable: z.boolean(),
  update: availableUpdateSchema.nullable(),
});

export const updateProgressEventSchema: z.ZodType<UpdateProgressEvent> = z.strictObject({
  version: semanticVersionSchema,
  phase: z.enum(["downloading", "downloaded", "installing"]),
  downloaded_bytes: z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER),
  total_bytes: z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER).nullable(),
});

export const projectMetadataSchema: z.ZodType<ProjectMetadata> = z.strictObject({
  description: z.string().nullable(),
  client: z.string().nullable(),
  location: z.string().nullable(),
});

export const projectSchema: z.ZodType<Project> = z.strictObject({
  id: projectIdSchema,
  name: z.string().trim().min(1),
  created_at: timestampSchema,
  updated_at: timestampSchema,
  archived_at: nullableTimestampSchema,
  metadata: projectMetadataSchema,
});

export const bootstrapStatusSchema: z.ZodType<BootstrapStatus> = z.strictObject({
  home_path: z.string().min(1),
  created_fresh: z.boolean(),
  structure_validated: z.boolean(),
  recovered_from_interrupt: z.boolean(),
  config_warnings: z.array(z.string()),
  database_migrations: z.array(z.string()),
  backups_created: z.array(safeAssetPathSchema),
});

export const toolManifestSchema: z.ZodType<ToolManifest> = z.strictObject({
  id: projectIdSchema,
  contract_version: z.number().int().positive(),
  tool_version: z
    .string()
    .regex(
      /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/,
      "expected a semantic version",
    ),
  name_key: z.string().regex(/^tools\..+\.name$/),
  description_key: z.string().regex(/^tools\..+\.description$/),
  icon: safeAssetPathSchema,
  route: z.string().regex(/^\/tools\/[a-z0-9]+(?:-[a-z0-9]+)*$/),
});

export const toolNavItemSchema: z.ZodType<ToolNavItem> = z.strictObject({
  tool_id: projectIdSchema,
  route: z.string().regex(/^\/tools\/[a-z0-9]+(?:-[a-z0-9]+)*$/),
  icon: safeAssetPathSchema,
  name_key: z.string().regex(/^tools\..+\.name$/),
  description_key: z.string().regex(/^tools\..+\.description$/),
});

const columnRoleSchema = z.enum([
  "item_number",
  "description",
  "unit",
  "quantity",
  "unit_price",
  "total_price",
  "currency",
  "notes",
  "unknown",
]);
const rowClassificationSchema = z.enum([
  "item",
  "heading",
  "subheading",
  "note",
  "subtotal",
  "total",
  "blank",
  "unknown",
]);
const columnRoleAssignmentSchema: z.ZodType<ColumnRoleAssignment> = z.strictObject({
  column_index: z.number().int().nonnegative(),
  column_letter: z.string().regex(/^[A-Z]{1,3}$/),
  role: columnRoleSchema,
  confidence: confidenceSchema,
});
const classifiedRowSchema: z.ZodType<ClassifiedRow> = z.strictObject({
  row_index: z.number().int().nonnegative(),
  classification: rowClassificationSchema,
  confidence: confidenceSchema,
});
const detectedTableSchema: z.ZodType<DetectedTable> = z.strictObject({
  sheet: z.string(),
  header_row: z.number().int().nonnegative().nullable(),
  start_row: z.number().int().nonnegative(),
  end_row: z.number().int().nonnegative(),
  columns: z.array(columnRoleAssignmentSchema),
  rows: z.array(classifiedRowSchema),
  interpretation_confidence: confidenceSchema,
  evidence: z.array(z.string()),
});
const sheetInventorySchema: z.ZodType<SheetInventory> = z.strictObject({
  index: z.number().int().nonnegative(),
  name: z.string(),
  visibility: z.enum(["visible", "hidden", "very_hidden"]),
  used_rows: z.number().int().nonnegative(),
  used_columns: z.number().int().nonnegative(),
  non_empty_cells: z.number().int().nonnegative(),
  detected_tables: z.number().int().nonnegative(),
});
export const workbookDiagnosticsSchema: z.ZodType<WorkbookDiagnostics> = z.strictObject({
  rule_set_version: z.string().min(1),
  sheets: z.array(sheetInventorySchema),
  tables: z.array(detectedTableSchema),
  interpretation_confidence: confidenceSchema,
  warnings: z.array(z.string()),
});
export const evidenceSchema: z.ZodType<Evidence> = z.strictObject({
  sheet: z.string(),
  cell: cellRefSchema.nullable(),
  range: cellRangeSchema.nullable(),
  description_key: z.string().nullable(),
  snippet: z.string().nullable(),
});
export const findingSchema: z.ZodType<Finding> = z.strictObject({
  id: uuidSchema,
  project_id: projectIdSchema,
  source_revision_id: uuidSchema,
  run_id: uuidSchema,
  rule_id: z.string().min(1),
  rule_set_version: z.string().min(1),
  category: z.enum([
    "arithmetic",
    "duplication",
    "omission",
    "inconsistency",
    "structure",
    "compliance",
    "other",
  ]),
  severity: z.enum(["info", "low", "medium", "high", "critical"]),
  confidence: confidenceSchema,
  title_key: z.string().regex(/^findings\./),
  title_params: z.record(z.string(), z.string()),
  explanation_key: z.string().regex(/^findings\./),
  explanation_params: z.record(z.string(), z.string()),
  suggested_action_key: z
    .string()
    .regex(/^findings\./)
    .nullable(),
  suggested_action_params: z.record(z.string(), z.string()),
  sheet: z.string().nullable(),
  cell: cellRefSchema.nullable(),
  range: cellRangeSchema.nullable(),
  source_row_id: z.string().nullable(),
  original_value: z.string().nullable(),
  original_formula: z.string().nullable(),
  evidence: z.array(evidenceSchema),
  origin: z.enum(["deterministic", "ai"]),
  created_at: timestampSchema,
});
const paretoAnalysisSchema: z.ZodType<ParetoAnalysis> = z.strictObject({
  context: z.string(),
  currency: z
    .string()
    .regex(/^[A-Z]{3}$/)
    .nullable(),
  total_amount: z.string(),
  top_item_count: z.number().int().nonnegative(),
  total_item_count: z.number().int().nonnegative(),
  cumulative_share_percent: z.string(),
});
const boqInspectorSummarySchema: z.ZodType<BoqInspectorSummary> = z.strictObject({
  item_rows: z.number().int().nonnegative(),
  finding_count: z.number().int().nonnegative(),
  pareto: z.array(paretoAnalysisSchema),
});
const boqNormalizedFactSchema: z.ZodType<BoqNormalizedFact> = z.strictObject({
  cell: cellRefSchema,
  raw: z.string(),
  formula: z.string().nullable(),
  normalized: z.string(),
});
const boqNormalizedRowSchema: z.ZodType<BoqNormalizedRow> = z.strictObject({
  source_row_id: z.string().min(1),
  sheet: z.string().min(1),
  source_row_number: z.number().int().positive(),
  classification: rowClassificationSchema,
  classification_confidence: confidenceSchema,
  section_path: z.array(z.string()),
  item_code: boqNormalizedFactSchema.nullable(),
  description: boqNormalizedFactSchema.nullable(),
  unit: boqNormalizedFactSchema.nullable(),
  quantity: boqNormalizedFactSchema.nullable(),
  rate_text: boqNormalizedFactSchema.nullable(),
  rate: boqNormalizedFactSchema.nullable(),
  amount: boqNormalizedFactSchema.nullable(),
  currency: boqNormalizedFactSchema.nullable(),
  error_cells: z.array(boqNormalizedFactSchema),
});
const boqAiPrioritizedRiskSchema: z.ZodType<BoqAiPrioritizedRisk> = z.strictObject({
  priority: z.enum(["high", "medium", "low"]),
  findingIds: z.array(z.string().min(1)).min(1).max(20),
  reason: z.string().trim().min(1).max(2_000),
  evidenceRefs: z.array(z.string().min(3).max(260)).max(100),
});
export const boqAiReviewSchema: z.ZodType<BoqAiReview> = z.strictObject({
  summary: z.string().trim().min(1).max(4_000),
  prioritizedRisks: z.array(boqAiPrioritizedRiskSchema).max(100),
  recommendations: z.array(z.string().trim().min(1).max(2_000)).max(100),
  rfiSuggestions: z.array(z.string().trim().min(1).max(2_000)).max(100),
  limitations: z.array(z.string().trim().min(1).max(2_000)).max(100),
  assumptions: z.array(z.string().trim().min(1).max(2_000)).max(100),
});

export const boqInspectorInputSchema: z.ZodType<BoqInspectorInput> = z.strictObject({
  source_revision_id: uuidSchema,
  rules: z.array(z.string().regex(/^[a-z0-9._-]{1,96}$/)),
});

export const boqInspectorSettingsSchema: z.ZodType<BoqInspectorSettings> = z.strictObject({
  locale: z.enum(["en", "ar"]),
  absolute_tolerance: decimalSchema,
  relative_tolerance: decimalSchema,
  decimal_precision: z.number().int().min(0).max(6),
  fuzzy_similarity_threshold_percent: z.number().int().min(50).max(100),
  low_confidence_threshold_percent: z.number().int().min(1).max(100),
});

export const boqInspectorOutputSchema: z.ZodType<BoqInspectorOutput> = z.strictObject({
  findings: z.array(findingSchema),
  diagnostics: workbookDiagnosticsSchema,
  summary: boqInspectorSummarySchema,
  normalized_rows: z.array(boqNormalizedRowSchema),
});

export const sourceRevisionSchema: z.ZodType<SourceRevision> = z.strictObject({
  id: uuidSchema,
  project_id: projectIdSchema,
  sha256: z.string().regex(/^[0-9a-f]{64}$/),
  original_filename: z.string().trim().min(1),
  original_path: z.string().min(1).nullable(),
  stored_path: safeAssetPathSchema,
  size_bytes: z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER),
  imported_at: timestampSchema,
  tool_id: projectIdSchema,
  workbook_metadata: z.unknown(),
});

export const analysisRunSchema: z.ZodType<AnalysisRun> = z.strictObject({
  id: uuidSchema,
  project_id: projectIdSchema,
  source_revision_id: uuidSchema,
  tool_id: projectIdSchema,
  tool_version: z.string().min(1),
  rule_set_version: z.string().min(1),
  app_version: z.string().min(1),
  status: z.enum(["pending", "running", "completed", "failed", "cancelled"]),
  started_at: timestampSchema,
  finished_at: nullableTimestampSchema,
  structure_diagnostics: workbookDiagnosticsSchema.nullable(),
  overall_confidence: confidenceSchema.nullable(),
});

export const runHistoryEntrySchema: z.ZodType<RunHistoryEntry> = z.strictObject({
  run: analysisRunSchema,
  source_sha256: z.string().regex(/^[0-9a-f]{64}$/),
  finding_count: z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER),
  export_count: z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER),
  ai_analysis_count: z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER),
  latest_ai_status: z.enum(["pending", "completed", "failed"]).nullable(),
});

export const exportRecordSchema: z.ZodType<ExportRecord> = z.strictObject({
  id: uuidSchema,
  run_id: uuidSchema,
  kind: z.enum(["xlsx", "pdf"]),
  language: z.enum(["en", "ar"]),
  relative_path: safeAssetPathSchema,
  sha256: z.string().regex(/^[0-9a-f]{64}$/),
  created_at: timestampSchema,
});

export const aiAnalysisSchema: z.ZodType<AiAnalysis> = z.strictObject({
  id: uuidSchema,
  run_id: uuidSchema,
  model: z.string().min(1),
  codex_version: z.string().min(1),
  language: z.enum(["en", "ar"]),
  input_scope_hash: z.string().regex(/^[0-9a-f]{64}$/),
  status: z.enum(["pending", "completed", "failed"]),
  validation_status: z.enum(["unvalidated", "validated", "rejected"]),
  grounding_status: z.enum(["pending", "validated", "rejected"]),
  output: z.unknown(),
  created_at: timestampSchema,
});

export const aiRuntimeStatusSchema: z.ZodType<AiRuntimeStatus> = z.strictObject({
  enabled: z.boolean(),
  bundled_runtime_available: z.boolean(),
  selected_runtime_available: z.boolean(),
  using_system_runtime: z.boolean(),
  codex_version: z.string().regex(/^\d+\.\d+\.\d+$/),
});

const aiPlanTypeSchema = z.enum([
  "free",
  "go",
  "plus",
  "pro",
  "prolite",
  "team",
  "self_serve_business_usage_based",
  "business",
  "enterprise_cbp_usage_based",
  "enterprise",
  "edu",
  "unknown",
]);

export const aiAccountSnapshotSchema: z.ZodType<AiAccountSnapshot> = z.strictObject({
  signed_in: z.boolean(),
  masked_email: z.string().nullable(),
  plan_type: aiPlanTypeSchema.nullable(),
  requires_openai_auth: z.boolean(),
  codex_version: z.string().regex(/^\d+\.\d+\.\d+$/),
});

export const aiLoginChallengeSchema: z.ZodType<AiLoginChallenge> = z.strictObject({
  login_id: z.string().regex(/^[A-Za-z0-9_-]{1,128}$/),
  mode: z.enum(["browser", "device_code"]),
  user_code: z.string().min(1).max(128).nullable(),
});

const aiRateLimitWindowSchema: z.ZodType<AiRateLimitWindow> = z.strictObject({
  used_percent: z.number().int().min(0).max(100),
  window_duration_minutes: z.number().int().nonnegative().nullable(),
  resets_at: z.number().int().nullable(),
});

export const aiRateLimitSnapshotSchema: z.ZodType<AiRateLimitSnapshot> = z.strictObject({
  primary: aiRateLimitWindowSchema.nullable(),
  secondary: aiRateLimitWindowSchema.nullable(),
  plan_type: aiPlanTypeSchema.nullable(),
  rate_limit_reached: z.boolean(),
  spend_control_reached: z.boolean(),
});

export const aiReviewScopeSchema: z.ZodType<AiReviewScope> = z.strictObject({
  run_id: uuidSchema,
  source_sha256: z.string().regex(/^[0-9a-f]{64}$/),
  source_row_count: z.number().int().nonnegative(),
  finding_count: z.number().int().nonnegative(),
  source_chunk_count: z.number().int().positive().max(4_294_967_295),
  planned_turn_count: z.number().int().positive().max(4_294_967_295),
  transmitted_bytes: z.number().int().nonnegative().max(4_294_967_295),
  input_scope_hash: z.string().regex(/^[0-9a-f]{64}$/),
});

export const runDetailsSchema: z.ZodType<RunDetails> = z.strictObject({
  run: analysisRunSchema,
  findings: z.array(findingSchema),
  output: z.unknown(),
  exports: z.array(exportRecordSchema),
  ai_analyses: z.array(aiAnalysisSchema),
});

export const ipcErrorSchema: z.ZodType<IpcError> = z.strictObject({
  code: z.string().regex(/^[A-Z][A-Z0-9_]*$/),
});

export const toolProgressSchema: z.ZodType<ToolProgress> = z.strictObject({
  phase_key: z.string().regex(/^tools\.[a-zA-Z0-9_.-]+$/),
  fraction: z.number().min(0).max(1),
  detail: z.string().nullable(),
});

export const toolProgressEventSchema: z.ZodType<ToolProgressEvent> = z.strictObject({
  run_id: uuidSchema,
  progress: toolProgressSchema,
});

export const toolRunResponseSchema: z.ZodType<ToolRunResponse> = z.strictObject({
  run: analysisRunSchema,
  output: z.unknown(),
});

export const boqToolRunResponseSchema = z.strictObject({
  run: analysisRunSchema,
  output: boqInspectorOutputSchema,
});

/** Strict frontend argument schemas for the current Tauri IPC commands. */
export const registerProjectArgsSchema = z.strictObject({
  id: projectIdSchema,
  name: z.string().trim().min(1),
});
export const archiveProjectArgsSchema = z.strictObject({ id: projectIdSchema });
export const listProjectsArgsSchema = z.strictObject({ include_archived: z.boolean() });
export const updateSettingsArgsSchema = z.strictObject({ patch: settingsPatchSchema });
export const importSourceArgsSchema = z.strictObject({
  project_id: projectIdSchema,
  tool_id: projectIdSchema,
  source_path: z.string().min(1),
});
export const quickImportSourceArgsSchema = z.strictObject({
  tool_id: projectIdSchema,
  source_path: z.string().min(1),
});
export const listSourceRevisionsArgsSchema = z.strictObject({
  project_id: projectIdSchema,
});
export const runToolArgsSchema = z.strictObject({
  run_id: uuidSchema,
  project_id: projectIdSchema,
  source_revision_id: uuidSchema,
  tool_id: projectIdSchema,
  input: z.unknown(),
  settings: z.unknown(),
});
export const boqRunToolArgsSchema = runToolArgsSchema.extend({
  input: boqInspectorInputSchema,
  settings: boqInspectorSettingsSchema,
});
export const cancelToolRunArgsSchema = z.strictObject({ run_id: uuidSchema });
export const listAnalysisRunsArgsSchema = z.strictObject({
  project_id: projectIdSchema,
});
export const openAnalysisRunArgsSchema = z.strictObject({ run_id: uuidSchema });
export const exportAnalysisRunArgsSchema = z.strictObject({
  run_id: uuidSchema,
  kind: z.enum(["xlsx", "pdf"]),
  language: z.enum(["en", "ar"]),
});
export const listRunExportsArgsSchema = z.strictObject({ run_id: uuidSchema });
export const revealExportArgsSchema = z.strictObject({
  run_id: uuidSchema,
  export_id: uuidSchema,
});
export const getAiAccountArgsSchema = z.strictObject({ refresh_token: z.boolean() });
export const startAiLoginArgsSchema = z.strictObject({
  mode: z.enum(["browser", "device_code"]),
});
export const cancelAiLoginArgsSchema = z.strictObject({
  login_id: z.string().regex(/^[A-Za-z0-9_-]{1,128}$/),
});
export const prepareAiReviewArgsSchema = z.strictObject({
  run_id: uuidSchema,
  language: z.enum(["en", "ar"]),
});
export const runAiReviewArgsSchema = prepareAiReviewArgsSchema.extend({
  input_scope_hash: z.string().regex(/^[0-9a-f]{64}$/),
  consent: z.literal(true),
});
export const cancelAiReviewArgsSchema = z.strictObject({ run_id: uuidSchema });
