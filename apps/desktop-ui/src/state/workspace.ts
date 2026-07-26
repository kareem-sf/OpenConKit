import { create } from "zustand";

import type {
  AnalysisRun,
  AppSettings,
  BootstrapStatus,
  ExportKind,
  ExportRecord,
  Project,
  RunDetails,
  RunHistoryEntry,
  SettingsPatch,
  SourceRevision,
  ToolManifest,
  ToolProgress,
  ToolProgressEvent,
  UpdateCheckResult,
} from "@openconkit/contracts";

import type { PreviewData as BrowserPreviewData } from "../dev/previewData";
import { desktopApi, desktopRuntimeAvailable, errorCodeOf } from "../lib/ipc";

interface WorkspaceState {
  initialized: boolean;
  loading: boolean;
  busyAction: "import" | "run" | "export" | "settings" | "reset" | null;
  errorCode: string | null;
  bootstrap: BootstrapStatus | null;
  settings: AppSettings | null;
  manifests: ToolManifest[];
  revisions: SourceRevision[];
  runs: AnalysisRun[];
  history: RunHistoryEntry[];
  runDetails: RunDetails | null;
  activeRunId: string | null;
  progress: ToolProgress | null;
  lastExport: ExportRecord | null;
  availableUpdate: UpdateCheckResult | null;
  initialize: () => Promise<void>;
  chooseAndImport: () => Promise<SourceRevision | null>;
  runRevision: (revision: SourceRevision) => Promise<RunDetails | null>;
  cancelActiveRun: () => Promise<void>;
  openRun: (runId: string) => Promise<RunDetails | null>;
  exportRun: (kind: ExportKind, language: "en" | "ar") => Promise<ExportRecord | null>;
  revealExport: (exportId: string) => Promise<boolean>;
  saveSettings: (patch: SettingsPatch) => Promise<AppSettings | null>;
  resetApplication: () => Promise<boolean>;
  receiveProgress: (event: ToolProgressEvent) => void;
  receiveAvailableUpdate: (update: UpdateCheckResult) => void;
  dismissAvailableUpdate: () => void;
  clearRunDetails: () => void;
  dismissError: () => void;
}

interface StorageGroupActivity {
  revisions: SourceRevision[];
  runs: AnalysisRun[];
  history: RunHistoryEntry[];
}

function browserPreviewActive(): boolean {
  return import.meta.env.DEV && !desktopRuntimeAvailable();
}

interface BrowserPreviewModule {
  previewData: () => BrowserPreviewData;
}

async function loadBrowserPreviewData(): Promise<BrowserPreviewData> {
  if (!import.meta.env.DEV) {
    throw new Error("Browser preview data is unavailable in production.");
  }
  const previewModulePath = "../dev/previewData.ts";
  const previewModule = (await import(
    /* @vite-ignore */ previewModulePath
  )) as BrowserPreviewModule;
  return previewModule.previewData();
}

async function storageGroupData(storageGroupId: string): Promise<StorageGroupActivity> {
  const [revisions, runs, history] = await Promise.all([
    desktopApi.listSourceRevisions(storageGroupId),
    desktopApi.listAnalysisRuns(storageGroupId),
    desktopApi.listRunHistory(storageGroupId),
  ]);
  return { revisions, runs, history };
}

async function loadLibrary(): Promise<Pick<WorkspaceState, "revisions" | "runs" | "history">> {
  const storageGroups = await desktopApi.listStorageGroups();
  const activities = await Promise.all(
    storageGroups.map((group: Project) => storageGroupData(group.id)),
  );
  return {
    revisions: activities
      .flatMap((activity) => activity.revisions)
      .sort((left, right) => left.imported_at.localeCompare(right.imported_at)),
    runs: activities
      .flatMap((activity) => activity.runs)
      .sort((left, right) => left.started_at.localeCompare(right.started_at)),
    history: activities
      .flatMap((activity) => activity.history)
      .sort((left, right) => left.run.started_at.localeCompare(right.run.started_at)),
  };
}

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  initialized: false,
  loading: false,
  busyAction: null,
  errorCode: null,
  bootstrap: null,
  settings: null,
  manifests: [],
  revisions: [],
  runs: [],
  history: [],
  runDetails: null,
  activeRunId: null,
  progress: null,
  lastExport: null,
  availableUpdate: null,

  initialize: async () => {
    if (get().loading || (get().initialized && get().settings !== null)) {
      return;
    }
    set({ loading: true, errorCode: null });
    if (browserPreviewActive()) {
      const preview = await loadBrowserPreviewData();
      set({
        initialized: true,
        loading: false,
        bootstrap: preview.bootstrap,
        settings: preview.settings,
        manifests: preview.manifests,
        revisions: preview.revisions,
        runs: preview.runs,
        history: preview.history,
        runDetails: preview.runDetails,
      });
      return;
    }
    const [bootstrap, settings, manifests, library] = await Promise.allSettled([
      desktopApi.bootstrapStatus(),
      desktopApi.getSettings(),
      desktopApi.listToolManifests(),
      loadLibrary(),
    ]);
    const rejected = [bootstrap, settings, manifests, library].find(
      (result): result is PromiseRejectedResult => result.status === "rejected",
    );
    const current = get();
    set({
      initialized: true,
      loading: false,
      errorCode: rejected ? errorCodeOf(rejected.reason) : null,
      bootstrap: bootstrap.status === "fulfilled" ? bootstrap.value : current.bootstrap,
      settings: settings.status === "fulfilled" ? settings.value : current.settings,
      manifests: manifests.status === "fulfilled" ? manifests.value : current.manifests,
      revisions: library.status === "fulfilled" ? library.value.revisions : current.revisions,
      runs: library.status === "fulfilled" ? library.value.runs : current.runs,
      history: library.status === "fulfilled" ? library.value.history : current.history,
    });
  },

  chooseAndImport: async () => {
    if (browserPreviewActive()) {
      set({ busyAction: null, errorCode: null });
      return null;
    }
    try {
      const sourcePath = await desktopApi.chooseWorkbook();
      if (!sourcePath) {
        return null;
      }
      set({ busyAction: "import", errorCode: null });
      const revision = await desktopApi.quickImportSource("boq-inspector", sourcePath);
      const library = await loadLibrary();
      set({ ...library, busyAction: null });
      return revision;
    } catch (error: unknown) {
      set({ busyAction: null, errorCode: errorCodeOf(error) });
      return null;
    }
  },

  runRevision: async (revision) => {
    const projectId = revision.project_id;
    const settings = get().settings;
    if (!settings) {
      set({ errorCode: "REPOSITORY_NOT_FOUND" });
      return null;
    }
    if (browserPreviewActive()) {
      const preview = await loadBrowserPreviewData();
      const runDetails =
        preview.runDetails.run.source_revision_id === revision.id ? preview.runDetails : null;
      if (!runDetails) {
        set({ busyAction: null, errorCode: "REPOSITORY_NOT_FOUND" });
        return null;
      }
      set({
        busyAction: null,
        activeRunId: null,
        progress: null,
        errorCode: null,
        revisions: preview.revisions,
        runs: preview.runs,
        history: preview.history,
        runDetails,
      });
      return runDetails;
    }
    const runId = crypto.randomUUID();
    set({
      busyAction: "run",
      activeRunId: runId,
      progress: {
        phase_key: "tools.boqInspector.progress.start",
        fraction: 0,
        detail: null,
      },
      errorCode: null,
      runDetails: null,
    });
    try {
      await desktopApi.runBoqInspector(runId, projectId, revision, settings);
      const [details, library] = await Promise.all([
        desktopApi.openAnalysisRun(runId),
        loadLibrary(),
      ]);
      set({
        busyAction: null,
        activeRunId: null,
        progress: null,
        runDetails: details,
        ...library,
      });
      return details;
    } catch (error: unknown) {
      const library = await loadLibrary().catch(() => ({
        revisions: get().revisions,
        runs: get().runs,
        history: get().history,
      }));
      set({
        busyAction: null,
        activeRunId: null,
        progress: null,
        ...library,
        errorCode: errorCodeOf(error),
      });
      return null;
    }
  },

  cancelActiveRun: async () => {
    const runId = get().activeRunId;
    if (!runId) {
      return;
    }
    if (browserPreviewActive()) {
      set({ busyAction: null, activeRunId: null, progress: null, errorCode: null });
      return;
    }
    try {
      await desktopApi.cancelToolRun(runId);
    } catch (error: unknown) {
      set({ errorCode: errorCodeOf(error) });
    }
  },

  openRun: async (runId) => {
    set({ loading: true, errorCode: null, lastExport: null });
    if (browserPreviewActive()) {
      const preview = await loadBrowserPreviewData();
      const runDetails = preview.runDetails.run.id === runId ? preview.runDetails : null;
      set({ loading: false, runDetails });
      if (!runDetails) {
        set({ errorCode: "REPOSITORY_NOT_FOUND" });
      }
      return runDetails;
    }
    try {
      const runDetails = await desktopApi.openAnalysisRun(runId);
      set({ loading: false, runDetails });
      return runDetails;
    } catch (error: unknown) {
      set({ loading: false, errorCode: errorCodeOf(error) });
      return null;
    }
  },

  exportRun: async (kind, language) => {
    const runId = get().runDetails?.run.id;
    if (!runId) {
      set({ errorCode: "REPOSITORY_NOT_FOUND" });
      return null;
    }
    if (browserPreviewActive()) {
      set({ busyAction: null, errorCode: null, lastExport: null });
      return null;
    }
    set({ busyAction: "export", errorCode: null, lastExport: null });
    try {
      const exported = await desktopApi.exportAnalysisRun(runId, kind, language);
      const current = get().runDetails;
      const [exports, library] = await Promise.all([
        desktopApi.listRunExports(runId),
        loadLibrary(),
      ]);
      set({
        busyAction: null,
        lastExport: exported,
        runDetails: current ? { ...current, exports } : current,
        ...library,
      });
      return exported;
    } catch (error: unknown) {
      set({ busyAction: null, errorCode: errorCodeOf(error) });
      return null;
    }
  },

  revealExport: async (exportId) => {
    const runId = get().runDetails?.run.id;
    if (!runId) {
      set({ errorCode: "REPOSITORY_NOT_FOUND" });
      return false;
    }
    if (browserPreviewActive()) {
      set({ errorCode: null });
      return false;
    }
    try {
      await desktopApi.revealExport(runId, exportId);
      return true;
    } catch (error: unknown) {
      set({ errorCode: errorCodeOf(error) });
      return false;
    }
  },

  saveSettings: async (patch) => {
    set({ busyAction: "settings", errorCode: null });
    if (browserPreviewActive()) {
      const current = get().settings;
      if (!current) {
        set({ busyAction: null, errorCode: "BACKGROUND_TASK_FAILED" });
        return null;
      }
      const settings: AppSettings = {
        ...current,
        onboarding_completed: patch.onboarding_completed ?? current.onboarding_completed,
        language: patch.language ?? current.language,
        theme: patch.theme ?? current.theme,
        update_channel: patch.update_channel ?? current.update_channel,
        tolerances: patch.tolerances ?? current.tolerances,
        privacy: patch.privacy ?? current.privacy,
        advanced: patch.advanced ?? current.advanced,
        last_successful_update_check:
          patch.last_successful_update_check ?? current.last_successful_update_check,
      };
      set({ busyAction: null, settings });
      return settings;
    }
    try {
      const settings = await desktopApi.updateSettings(patch);
      set({ busyAction: null, settings });
      return settings;
    } catch (error: unknown) {
      set({ busyAction: null, errorCode: errorCodeOf(error) });
      return null;
    }
  },

  resetApplication: async () => {
    set({ busyAction: "reset", errorCode: null });
    if (browserPreviewActive()) {
      const preview = await loadBrowserPreviewData();
      set({
        initialized: true,
        loading: false,
        busyAction: null,
        bootstrap: {
          ...preview.bootstrap,
          created_fresh: true,
          database_migrations: [],
          backups_created: [],
        },
        settings: { ...preview.settings, onboarding_completed: false },
        manifests: preview.manifests,
        revisions: [],
        runs: [],
        history: [],
        runDetails: null,
        activeRunId: null,
        progress: null,
        lastExport: null,
        availableUpdate: null,
      });
      window.location.hash = "#/";
      return true;
    }
    try {
      await desktopApi.resetOpenConKit("RESET");
      return true;
    } catch (error: unknown) {
      set({ busyAction: null, errorCode: errorCodeOf(error) });
      return false;
    }
  },

  receiveProgress: (event) => {
    if (event.run_id === get().activeRunId) {
      set({ progress: event.progress });
    }
  },

  receiveAvailableUpdate: (availableUpdate) => set({ availableUpdate }),
  dismissAvailableUpdate: () => set({ availableUpdate: null }),

  clearRunDetails: () => set({ runDetails: null, lastExport: null }),
  dismissError: () => set({ errorCode: null }),
}));
