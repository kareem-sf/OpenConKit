import { describe, expect, it } from "vitest";

import {
  aiAccountSnapshotSchema,
  aiRateLimitSnapshotSchema,
  aiReviewScopeSchema,
  appSettingsSchema,
  boqAiReviewSchema,
  boqInspectorInputSchema,
  boqInspectorOutputSchema,
  boqInspectorSettingsSchema,
  boqRunToolArgsSchema,
  cancelAiReviewArgsSchema,
  boqToolRunResponseSchema,
  exportAnalysisRunArgsSchema,
  importSourceArgsSchema,
  ipcErrorSchema,
  projectIdSchema,
  registerProjectArgsSchema,
  revealExportArgsSchema,
  runAiReviewArgsSchema,
  runDetailsSchema,
  settingsPatchSchema,
  startAiLoginArgsSchema,
  sourceRevisionSchema,
  toolProgressEventSchema,
  toolManifestSchema,
  updateCheckResultSchema,
  updateProgressEventSchema,
} from "./schemas";

describe("IPC schemas", () => {
  it("accepts the canonical default settings shape", () => {
    expect(
      appSettingsSchema.parse({
        schema_version: 2,
        onboarding_completed: false,
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
      }),
    ).toBeDefined();
  });

  it("rejects unknown fields and invalid domain values", () => {
    expect(projectIdSchema.safeParse("../escape").success).toBe(false);
    expect(
      registerProjectArgsSchema.safeParse({
        id: "tower-a",
        name: "Tower A",
        unexpected: true,
      }).success,
    ).toBe(false);
    expect(
      settingsPatchSchema.safeParse({
        onboarding_completed: null,
        language: null,
        theme: null,
        update_channel: null,
        tolerances: {
          absolute_tolerance: "-1",
          relative_tolerance: "0",
          decimal_precision: 2,
        },
        privacy: null,
        advanced: null,
        last_successful_update_check: null,
      }).success,
    ).toBe(false);
  });

  it("accepts coded IPC errors without backend diagnostics", () => {
    expect(ipcErrorSchema.parse({ code: "TOOL_CANCELLED" })).toEqual({
      code: "TOOL_CANCELLED",
    });
    expect(
      ipcErrorSchema.safeParse({
        code: "STORAGE_FAILED",
        message: "C:\\Users\\person\\sensitive.xlsx",
      }).success,
    ).toBe(false);
  });

  it("validates bounded updater metadata and allowlisted manual URLs", () => {
    const result = {
      checked_at: "2026-07-24T08:00:00Z",
      channel: "stable",
      current_version: "1.0.0",
      portable: true,
      update: {
        version: "1.1.0",
        notes: "Security and reliability fixes.",
        published_at: "2026-07-24T07:00:00Z",
        size_bytes: 42_000_000,
        can_install: false,
        manual_download_url: "https://github.com/kareem-sf/OpenConKit/releases/tag/v1.1.0",
      },
    };
    expect(updateCheckResultSchema.safeParse(result).success).toBe(true);
    expect(
      updateCheckResultSchema.safeParse({
        ...result,
        update: {
          ...result.update,
          manual_download_url: "https://attacker.invalid/OpenConKit.exe",
        },
      }).success,
    ).toBe(false);
    expect(
      updateCheckResultSchema.safeParse({
        ...result,
        update: { ...result.update, notes: "x".repeat(16_385) },
      }).success,
    ).toBe(false);
  });

  it("rejects unsafe updater progress numbers", () => {
    expect(
      updateProgressEventSchema.safeParse({
        version: "1.1.0",
        phase: "downloading",
        downloaded_bytes: 1024,
        total_bytes: 2048,
      }).success,
    ).toBe(true);
    expect(
      updateProgressEventSchema.safeParse({
        version: "1.1.0",
        phase: "extracting",
        downloaded_bytes: -1,
        total_bytes: null,
      }).success,
    ).toBe(false);
  });

  it("validates tool identity, route, asset path, and semantic version", () => {
    const manifest = {
      id: "boq-inspector",
      contract_version: 2,
      tool_version: "0.0.1",
      name_key: "tools.boqInspector.name",
      description_key: "tools.boqInspector.description",
      icon: "tools/boq-inspector.svg",
      route: "/tools/boq-inspector",
    };
    expect(toolManifestSchema.safeParse(manifest).success).toBe(true);
    expect(toolManifestSchema.safeParse({ ...manifest, icon: "../secret" }).success).toBe(false);
    expect(toolManifestSchema.safeParse({ ...manifest, tool_version: "latest" }).success).toBe(
      false,
    );
  });

  it("validates BOQ engine inputs, settings, and strict output shape", () => {
    expect(
      boqInspectorInputSchema.safeParse({
        source_revision_id: "00000000-0000-4000-8000-000000000001",
        rules: ["boq.amount_mismatch"],
      }).success,
    ).toBe(true);
    expect(
      boqInspectorSettingsSchema.safeParse({
        locale: "ar",
        absolute_tolerance: "0.01",
        relative_tolerance: "0.001",
        decimal_precision: 2,
        fuzzy_similarity_threshold_percent: 85,
        low_confidence_threshold_percent: 65,
      }).success,
    ).toBe(true);
    expect(
      boqInspectorOutputSchema.safeParse({
        findings: [],
        diagnostics: {
          rule_set_version: "2026.07.2",
          sheets: [],
          tables: [],
          interpretation_confidence: 0,
          warnings: ["no_tables_detected"],
        },
        summary: {
          item_rows: 0,
          finding_count: 0,
          pareto: [],
        },
        normalized_rows: [],
      }).success,
    ).toBe(true);
    expect(
      boqInspectorSettingsSchema.safeParse({
        locale: "en",
        absolute_tolerance: "-1",
        relative_tolerance: "0.001",
        decimal_precision: 7,
        fuzzy_similarity_threshold_percent: 10,
        low_confidence_threshold_percent: 0,
      }).success,
    ).toBe(false);
  });

  it("validates run lifecycle payloads and rejects malformed IPC arguments", () => {
    const run = {
      id: "00000000-0000-4000-8000-000000000011",
      project_id: "tower-a",
      source_revision_id: "00000000-0000-4000-8000-000000000001",
      tool_id: "boq-inspector",
      tool_version: "0.0.1",
      rule_set_version: "2026.07.2",
      app_version: "0.0.1",
      status: "completed",
      started_at: "2026-07-23T10:00:00Z",
      finished_at: "2026-07-23T10:00:01Z",
      structure_diagnostics: {
        rule_set_version: "2026.07.2",
        sheets: [],
        tables: [],
        interpretation_confidence: 0.9,
        warnings: [],
      },
      overall_confidence: 0.9,
    };
    const output = {
      findings: [],
      diagnostics: run.structure_diagnostics,
      summary: {
        item_rows: 12,
        finding_count: 0,
        pareto: [],
      },
      normalized_rows: [],
    };

    expect(boqToolRunResponseSchema.safeParse({ run, output }).success).toBe(true);
    expect(
      runDetailsSchema.safeParse({
        run,
        findings: [],
        output,
        exports: [],
        ai_analyses: [],
      }).success,
    ).toBe(true);
    expect(
      exportAnalysisRunArgsSchema.safeParse({
        run_id: run.id,
        kind: "pdf",
        language: "ar",
      }).success,
    ).toBe(true);
    expect(
      exportAnalysisRunArgsSchema.safeParse({
        run_id: run.id,
        kind: "docx",
        language: "fr",
      }).success,
    ).toBe(false);
    expect(
      revealExportArgsSchema.safeParse({
        run_id: run.id,
        export_id: "00000000-0000-4000-8000-000000000099",
      }).success,
    ).toBe(true);
    expect(
      revealExportArgsSchema.safeParse({
        run_id: run.id,
        export_id: "../report.pdf",
      }).success,
    ).toBe(false);
    expect(
      toolProgressEventSchema.safeParse({
        run_id: run.id,
        progress: {
          phase_key: "tools.boqInspector.progress.analyze",
          fraction: 0.5,
          detail: null,
        },
      }).success,
    ).toBe(true);
    expect(
      boqRunToolArgsSchema.safeParse({
        run_id: run.id,
        project_id: "tower-a",
        source_revision_id: run.source_revision_id,
        tool_id: "boq-inspector",
        input: {
          source_revision_id: run.source_revision_id,
          rules: [],
        },
        settings: {
          locale: "en",
          absolute_tolerance: "0.01",
          relative_tolerance: "0.001",
          decimal_precision: 2,
          fuzzy_similarity_threshold_percent: 85,
          low_confidence_threshold_percent: 65,
        },
      }).success,
    ).toBe(true);
    expect(
      importSourceArgsSchema.safeParse({
        project_id: "tower-a",
        tool_id: "boq-inspector",
        source_path: "",
      }).success,
    ).toBe(false);
  });

  it("treats bounded file sizes as JavaScript numbers", () => {
    expect(
      sourceRevisionSchema.safeParse({
        id: "00000000-0000-4000-8000-000000000001",
        project_id: "tower-a",
        sha256: "a".repeat(64),
        original_filename: "boq.xlsx",
        original_path: "C:\\Tender\\boq.xlsx",
        stored_path: "projects/tower-a/sources/revision/boq.xlsx",
        size_bytes: 67_108_864,
        imported_at: "2026-07-23T10:00:00Z",
        tool_id: "boq-inspector",
        workbook_metadata: {},
      }).success,
    ).toBe(true);
  });

  it("validates safe grounded-AI payloads and rejects unsafe consent shapes", () => {
    expect(
      aiAccountSnapshotSchema.safeParse({
        signed_in: true,
        masked_email: "k***@example.com",
        plan_type: "plus",
        requires_openai_auth: true,
        codex_version: "0.145.0",
      }).success,
    ).toBe(true);
    expect(
      aiAccountSnapshotSchema.safeParse({
        signed_in: true,
        masked_email: "person@example.com",
        plan_type: "plus",
        requires_openai_auth: true,
        codex_version: "0.145.0",
        access_token: "must-not-cross-ipc",
      }).success,
    ).toBe(false);

    expect(
      aiRateLimitSnapshotSchema.safeParse({
        primary: {
          used_percent: 25,
          window_duration_minutes: 300,
          resets_at: 1_785_000_000,
        },
        secondary: null,
        plan_type: "plus",
        rate_limit_reached: false,
        spend_control_reached: false,
      }).success,
    ).toBe(true);

    const runId = "00000000-0000-4000-8000-000000000011";
    const scopeHash = "b".repeat(64);
    expect(
      aiReviewScopeSchema.safeParse({
        run_id: runId,
        source_sha256: "a".repeat(64),
        source_row_count: 12,
        finding_count: 2,
        source_chunk_count: 1,
        planned_turn_count: 1,
        transmitted_bytes: 4096,
        input_scope_hash: scopeHash,
      }).success,
    ).toBe(true);
    expect(
      runAiReviewArgsSchema.safeParse({
        run_id: runId,
        language: "ar",
        input_scope_hash: scopeHash,
        consent: true,
      }).success,
    ).toBe(true);
    expect(
      runAiReviewArgsSchema.safeParse({
        run_id: runId,
        language: "ar",
        input_scope_hash: scopeHash,
        consent: false,
      }).success,
    ).toBe(false);
    expect(startAiLoginArgsSchema.safeParse({ mode: "browser" }).success).toBe(true);
    expect(cancelAiReviewArgsSchema.safeParse({ run_id: runId }).success).toBe(true);

    expect(
      boqAiReviewSchema.safeParse({
        summary: "Two deterministic findings deserve review.",
        prioritizedRisks: [
          {
            priority: "high",
            findingIds: ["00000000-0000-4000-8000-000000000012"],
            reason: "The supported amount mismatch is material.",
            evidenceRefs: ["Sheet1!D12"],
          },
        ],
        recommendations: ["Review the cited deterministic finding."],
        rfiSuggestions: ["Confirm the intended rate for Sheet1 row 12."],
        limitations: ["No contract terms were supplied."],
        assumptions: ["The configured currency applies."],
      }).success,
    ).toBe(true);
  });
});
