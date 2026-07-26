import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { z } from "zod";

import {
  aiAccountSnapshotSchema,
  aiAnalysisSchema,
  aiLoginChallengeSchema,
  aiRateLimitSnapshotSchema,
  aiReviewScopeSchema,
  aiRuntimeStatusSchema,
  analysisRunSchema,
  appSettingsSchema,
  bootstrapStatusSchema,
  boqToolRunResponseSchema,
  exportRecordSchema,
  ipcErrorSchema,
  projectSchema,
  runDetailsSchema,
  runHistoryEntrySchema,
  sourceRevisionSchema,
  toolManifestSchema,
  toolProgressEventSchema,
  updateCheckResultSchema,
  updateProgressEventSchema,
  type AnalysisRun,
  type AiAccountSnapshot,
  type AiAnalysis,
  type AiLoginChallenge,
  type AiLoginMode,
  type AiRateLimitSnapshot,
  type AiReviewScope,
  type AiRuntimeStatus,
  type AppSettings,
  type BootstrapStatus,
  type ExportKind,
  type ExportRecord,
  type Project,
  type RunDetails,
  type RunHistoryEntry,
  type SettingsPatch,
  type SourceRevision,
  type ToolManifest,
  type ToolProgressEvent,
  type ToolRunResponse,
  type UpdateChannel,
  type UpdateCheckResult,
  type UpdateProgressEvent,
} from "@openconkit/contracts";

import { i18n } from "../i18n";

const storageGroupsSchema = z.array(projectSchema);
const revisionsSchema = z.array(sourceRevisionSchema);
const runsSchema = z.array(analysisRunSchema);
const historySchema = z.array(runHistoryEntrySchema);
const exportsSchema = z.array(exportRecordSchema);
const manifestsSchema = z.array(toolManifestSchema);

/**
 * A command error whose code is safe to localize and display. Privileged
 * backend diagnostics deliberately do not cross into the WebView.
 */
export class IpcCommandError extends Error {
  public readonly code: string;

  public constructor(code: string) {
    super(code);
    this.name = "IpcCommandError";
    this.code = code;
  }
}

async function command<T>(
  name: string,
  args: Record<string, unknown> | undefined,
  schema: z.ZodType<T>,
): Promise<T> {
  try {
    const result = await invoke<unknown>(name, args);
    const parsed = schema.safeParse(result);
    if (!parsed.success) {
      throw new IpcCommandError("IPC_RESPONSE_INVALID");
    }
    return parsed.data;
  } catch (error: unknown) {
    throw normalizeCommandError(error);
  }
}

function normalizeCommandError(error: unknown): IpcCommandError {
  if (error instanceof IpcCommandError) {
    return error;
  }
  const direct = ipcErrorSchema.safeParse(error);
  if (direct.success) {
    return new IpcCommandError(direct.data.code);
  }
  if (typeof error === "string") {
    try {
      const parsed: unknown = JSON.parse(error);
      const serialized = ipcErrorSchema.safeParse(parsed);
      if (serialized.success) {
        return new IpcCommandError(serialized.data.code);
      }
    } catch {
      // Tauri may reject with a plain host string on a transport failure.
    }
  }
  return new IpcCommandError("BACKGROUND_TASK_FAILED");
}

export function errorCodeOf(error: unknown): string {
  return error instanceof IpcCommandError ? error.code : "BACKGROUND_TASK_FAILED";
}

export function desktopRuntimeAvailable(): boolean {
  return isTauri();
}

export const desktopApi = {
  appVersion(): Promise<string> {
    return command("app_version", undefined, z.string().min(1));
  },

  bootstrapStatus(): Promise<BootstrapStatus> {
    return command("bootstrap_status", undefined, bootstrapStatusSchema);
  },

  getSettings(): Promise<AppSettings> {
    return command("get_settings", undefined, appSettingsSchema);
  },

  updateSettings(patch: SettingsPatch): Promise<AppSettings> {
    return command("update_settings", { patch }, appSettingsSchema);
  },

  async resetOpenConKit(confirmation: string): Promise<void> {
    await command("reset_openconkit", { confirmation }, z.null());
  },

  checkForUpdates(): Promise<UpdateCheckResult> {
    return command("check_for_updates", undefined, updateCheckResultSchema);
  },

  async installUpdate(expectedVersion: string, channel: UpdateChannel): Promise<void> {
    await command("install_update", { expected_version: expectedVersion, channel }, z.null());
  },

  async openUpdateDownload(expectedVersion: string): Promise<void> {
    await command("open_update_download", { expected_version: expectedVersion }, z.null());
  },

  aiRuntimeStatus(): Promise<AiRuntimeStatus> {
    return command("ai_runtime_status", undefined, aiRuntimeStatusSchema);
  },

  getAiAccount(refreshToken = false): Promise<AiAccountSnapshot> {
    return command("get_ai_account", { refresh_token: refreshToken }, aiAccountSnapshotSchema);
  },

  startAiLogin(mode: AiLoginMode): Promise<AiLoginChallenge> {
    return command("start_ai_login", { mode }, aiLoginChallengeSchema);
  },

  async cancelAiLogin(loginId: string): Promise<void> {
    await command("cancel_ai_login", { login_id: loginId }, z.null());
  },

  async logoutAi(): Promise<void> {
    await command("logout_ai", undefined, z.null());
  },

  getAiRateLimits(): Promise<AiRateLimitSnapshot> {
    return command("get_ai_rate_limits", undefined, aiRateLimitSnapshotSchema);
  },

  prepareAiReview(runId: string, language: "en" | "ar"): Promise<AiReviewScope> {
    return command("prepare_ai_review", { run_id: runId, language }, aiReviewScopeSchema);
  },

  runAiReview(runId: string, language: "en" | "ar", inputScopeHash: string): Promise<AiAnalysis> {
    return command(
      "run_ai_review",
      {
        run_id: runId,
        language,
        input_scope_hash: inputScopeHash,
        consent: true,
      },
      aiAnalysisSchema,
    );
  },

  cancelAiReview(runId: string): Promise<boolean> {
    return command("cancel_ai_review", { run_id: runId }, z.boolean());
  },

  /**
   * Loads legacy persistence groups so existing workbook revisions and runs
   * remain accessible after the Projects UI was removed.
   */
  listStorageGroups(includeArchived = false): Promise<Project[]> {
    return command(
      "list_storage_groups",
      { include_archived: includeArchived },
      storageGroupsSchema,
    );
  },

  listSourceRevisions(projectId: string): Promise<SourceRevision[]> {
    return command("list_source_revisions", { project_id: projectId }, revisionsSchema);
  },

  listAnalysisRuns(projectId: string): Promise<AnalysisRun[]> {
    return command("list_analysis_runs", { project_id: projectId }, runsSchema);
  },

  listRunHistory(projectId: string): Promise<RunHistoryEntry[]> {
    return command("list_run_history", { project_id: projectId }, historySchema);
  },

  openAnalysisRun(runId: string): Promise<RunDetails> {
    return command("open_analysis_run", { run_id: runId }, runDetailsSchema);
  },

  listRunExports(runId: string): Promise<ExportRecord[]> {
    return command("list_run_exports", { run_id: runId }, exportsSchema);
  },

  exportAnalysisRun(runId: string, kind: ExportKind, language: "en" | "ar"): Promise<ExportRecord> {
    return command("export_analysis_run", { run_id: runId, kind, language }, exportRecordSchema);
  },

  async revealExport(runId: string, exportId: string): Promise<void> {
    await command("reveal_export", { run_id: runId, export_id: exportId }, z.null());
  },

  quickImportSource(toolId: string, sourcePath: string): Promise<SourceRevision> {
    return command(
      "quick_import_source",
      { tool_id: toolId, source_path: sourcePath },
      sourceRevisionSchema,
    );
  },

  async chooseWorkbook(): Promise<string | null> {
    const selected = await open({
      multiple: false,
      directory: false,
      title: i18n.t("workbooks.importWorkbook"),
      filters: [{ name: i18n.t("workbooks.dialogFilter"), extensions: ["xls", "xlsx"] }],
    });
    return typeof selected === "string" ? selected : null;
  },

  async chooseSystemCodex(): Promise<string | null> {
    const selected = await open({
      multiple: false,
      directory: false,
      title: i18n.t("settings.chooseSystemCodex"),
    });
    return typeof selected === "string" ? selected : null;
  },

  runBoqInspector(
    runId: string,
    projectId: string,
    revision: SourceRevision,
    settings: AppSettings,
  ): Promise<ToolRunResponse> {
    const locale = settings.language === "ar" ? "ar" : "en";
    return command(
      "run_tool",
      {
        run_id: runId,
        project_id: projectId,
        source_revision_id: revision.id,
        tool_id: revision.tool_id,
        input: { source_revision_id: revision.id, rules: [] },
        settings: {
          locale,
          absolute_tolerance: settings.tolerances.absolute_tolerance,
          relative_tolerance: settings.tolerances.relative_tolerance,
          decimal_precision: settings.tolerances.decimal_precision,
          fuzzy_similarity_threshold_percent: 85,
          low_confidence_threshold_percent: 65,
        },
      },
      boqToolRunResponseSchema,
    );
  },

  cancelToolRun(runId: string): Promise<boolean> {
    return command("cancel_tool_run", { run_id: runId }, z.boolean());
  },

  listToolManifests(): Promise<ToolManifest[]> {
    return command("list_tool_manifests", undefined, manifestsSchema);
  },

  async onToolProgress(handler: (event: ToolProgressEvent) => void): Promise<UnlistenFn> {
    return listen<unknown>("tool-progress", (event) => {
      const parsed = toolProgressEventSchema.safeParse(event.payload);
      if (parsed.success) {
        handler(parsed.data);
      }
    });
  },

  async onUpdateProgress(handler: (event: UpdateProgressEvent) => void): Promise<UnlistenFn> {
    return listen<unknown>("update-progress", (event) => {
      const parsed = updateProgressEventSchema.safeParse(event.payload);
      if (parsed.success) {
        handler(parsed.data);
      }
    });
  },

  async onUpdateAvailable(handler: (event: UpdateCheckResult) => void): Promise<UnlistenFn> {
    return listen<unknown>("update-available", (event) => {
      const parsed = updateCheckResultSchema.safeParse(event.payload);
      if (parsed.success && parsed.data.update) {
        handler(parsed.data);
      }
    });
  },
};
