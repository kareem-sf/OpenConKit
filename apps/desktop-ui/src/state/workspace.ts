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

import { desktopApi, desktopRuntimeAvailable, errorCodeOf } from "../lib/ipc";

interface WorkspaceState {
  initialized: boolean;
  loading: boolean;
  busyAction: "project" | "import" | "run" | "export" | "settings" | null;
  errorCode: string | null;
  bootstrap: BootstrapStatus | null;
  settings: AppSettings | null;
  manifests: ToolManifest[];
  projects: Project[];
  projectActivity: Record<string, ProjectActivity>;
  selectedProjectId: string | null;
  revisions: SourceRevision[];
  runs: AnalysisRun[];
  history: RunHistoryEntry[];
  runDetails: RunDetails | null;
  activeRunId: string | null;
  progress: ToolProgress | null;
  lastExport: ExportRecord | null;
  availableUpdate: UpdateCheckResult | null;
  initialize: () => Promise<void>;
  refreshProjects: () => Promise<void>;
  selectProject: (projectId: string) => Promise<void>;
  createProject: (id: string, name: string) => Promise<Project | null>;
  archiveProject: (projectId: string) => Promise<boolean>;
  chooseAndImport: () => Promise<SourceRevision | null>;
  runRevision: (revision: SourceRevision) => Promise<RunDetails | null>;
  cancelActiveRun: () => Promise<void>;
  openRun: (runId: string) => Promise<RunDetails | null>;
  exportRun: (kind: ExportKind, language: "en" | "ar") => Promise<ExportRecord | null>;
  revealExport: (exportId: string) => Promise<boolean>;
  saveSettings: (patch: SettingsPatch) => Promise<AppSettings | null>;
  receiveProgress: (event: ToolProgressEvent) => void;
  receiveAvailableUpdate: (update: UpdateCheckResult) => void;
  dismissAvailableUpdate: () => void;
  clearRunDetails: () => void;
  dismissError: () => void;
}

interface ProjectActivity {
  revisions: SourceRevision[];
  runs: AnalysisRun[];
  history: RunHistoryEntry[];
}

async function projectData(projectId: string) {
  const [revisions, runs, history] = await Promise.all([
    desktopApi.listSourceRevisions(projectId),
    desktopApi.listAnalysisRuns(projectId),
    desktopApi.listRunHistory(projectId),
  ]);
  return { revisions, runs, history };
}

async function allProjectData(projects: Project[]): Promise<Record<string, ProjectActivity>> {
  const entries = await Promise.all(
    projects.map(async (project) => [project.id, await projectData(project.id)] as const),
  );
  return Object.fromEntries(entries);
}

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  initialized: false,
  loading: false,
  busyAction: null,
  errorCode: null,
  bootstrap: null,
  settings: null,
  manifests: [],
  projects: [],
  projectActivity: {},
  selectedProjectId: null,
  revisions: [],
  runs: [],
  history: [],
  runDetails: null,
  activeRunId: null,
  progress: null,
  lastExport: null,
  availableUpdate: null,

  initialize: async () => {
    if (get().loading || get().initialized) {
      return;
    }
    set({ loading: true, errorCode: null });
    const e2eMode =
      import.meta.env.DEV &&
      new URLSearchParams(window.location.search).get("openconkit-e2e") === "1";
    if (import.meta.env.DEV && !e2eMode && !desktopRuntimeAvailable()) {
      const { previewData } = await import("../dev/previewData");
      const preview = previewData();
      const selectedProjectId = preview.projects[0]?.id ?? null;
      const data = selectedProjectId
        ? (preview.projectActivity[selectedProjectId] ?? {
            revisions: [],
            runs: [],
            history: [],
          })
        : { revisions: [], runs: [], history: [] };
      set({
        initialized: true,
        loading: false,
        bootstrap: preview.bootstrap,
        settings: preview.settings,
        manifests: preview.manifests,
        projects: preview.projects,
        projectActivity: preview.projectActivity,
        selectedProjectId,
        revisions: data.revisions,
        runs: data.runs,
        history: data.history,
        runDetails: preview.runDetails,
      });
      return;
    }
    try {
      const [bootstrap, settings, manifests, projects] = await Promise.all([
        desktopApi.bootstrapStatus(),
        desktopApi.getSettings(),
        desktopApi.listToolManifests(),
        desktopApi.listProjects(),
      ]);
      const projectActivity = await allProjectData(projects);
      const selectedProjectId = projects[0]?.id ?? null;
      const data = selectedProjectId
        ? (projectActivity[selectedProjectId] ?? { revisions: [], runs: [], history: [] })
        : { revisions: [], runs: [], history: [] };
      set({
        initialized: true,
        loading: false,
        bootstrap,
        settings,
        manifests,
        projects,
        projectActivity,
        selectedProjectId,
        revisions: data.revisions,
        runs: data.runs,
        history: data.history,
      });
    } catch (error: unknown) {
      set({
        initialized: true,
        loading: false,
        errorCode: errorCodeOf(error),
      });
    }
  },

  refreshProjects: async () => {
    try {
      set({ errorCode: null });
      const projects = await desktopApi.listProjects();
      const projectActivity = await allProjectData(projects);
      set({ projects, projectActivity });
    } catch (error: unknown) {
      set({ errorCode: errorCodeOf(error) });
    }
  },

  selectProject: async (projectId) => {
    const cached = get().projectActivity[projectId];
    set({
      selectedProjectId: projectId,
      revisions: cached?.revisions ?? [],
      runs: cached?.runs ?? [],
      history: cached?.history ?? [],
      runDetails: null,
      lastExport: null,
      loading: true,
      errorCode: null,
    });
    try {
      const data = await projectData(projectId);
      if (get().selectedProjectId === projectId) {
        set((state) => ({
          ...data,
          loading: false,
          projectActivity: { ...state.projectActivity, [projectId]: data },
        }));
      }
    } catch (error: unknown) {
      set({ loading: false, errorCode: errorCodeOf(error) });
    }
  },

  createProject: async (id, name) => {
    set({ busyAction: "project", errorCode: null });
    try {
      const project = await desktopApi.registerProject(id, name);
      const projects = await desktopApi.listProjects();
      set({
        busyAction: null,
        projects,
        projectActivity: {
          ...get().projectActivity,
          [project.id]: { revisions: [], runs: [], history: [] },
        },
        selectedProjectId: project.id,
        revisions: [],
        runs: [],
        history: [],
        runDetails: null,
      });
      return project;
    } catch (error: unknown) {
      set({ busyAction: null, errorCode: errorCodeOf(error) });
      return null;
    }
  },

  archiveProject: async (projectId) => {
    set({ busyAction: "project", errorCode: null });
    try {
      await desktopApi.archiveProject(projectId);
      const projects = await desktopApi.listProjects();
      const projectActivity = await allProjectData(projects);
      const selectedProjectId = projects[0]?.id ?? null;
      const data = selectedProjectId
        ? (projectActivity[selectedProjectId] ?? { revisions: [], runs: [], history: [] })
        : { revisions: [], runs: [], history: [] };
      set({
        busyAction: null,
        projects,
        projectActivity,
        selectedProjectId,
        revisions: data.revisions,
        runs: data.runs,
        history: data.history,
        runDetails: null,
      });
      return true;
    } catch (error: unknown) {
      set({ busyAction: null, errorCode: errorCodeOf(error) });
      return false;
    }
  },

  chooseAndImport: async () => {
    const projectId = get().selectedProjectId;
    if (!projectId) {
      set({ errorCode: "REPOSITORY_NOT_FOUND" });
      return null;
    }
    try {
      const sourcePath = await desktopApi.chooseWorkbook();
      if (!sourcePath) {
        return null;
      }
      set({ busyAction: "import", errorCode: null });
      const revision = await desktopApi.importSource(projectId, "boq-inspector", sourcePath);
      const data = await projectData(projectId);
      set((state) => ({
        ...data,
        busyAction: null,
        projectActivity: { ...state.projectActivity, [projectId]: data },
      }));
      return revision;
    } catch (error: unknown) {
      set({ busyAction: null, errorCode: errorCodeOf(error) });
      return null;
    }
  },

  runRevision: async (revision) => {
    const projectId = get().selectedProjectId;
    const settings = get().settings;
    if (!projectId || !settings) {
      set({ errorCode: "REPOSITORY_NOT_FOUND" });
      return null;
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
      const [details, runs, history] = await Promise.all([
        desktopApi.openAnalysisRun(runId),
        desktopApi.listAnalysisRuns(projectId),
        desktopApi.listRunHistory(projectId),
      ]);
      set({
        busyAction: null,
        activeRunId: null,
        progress: null,
        runDetails: details,
        runs,
        history,
        projectActivity: {
          ...get().projectActivity,
          [projectId]: { revisions: get().revisions, runs, history },
        },
      });
      return details;
    } catch (error: unknown) {
      const [runs, history] = await Promise.all([
        desktopApi.listAnalysisRuns(projectId).catch(() => get().runs),
        desktopApi.listRunHistory(projectId).catch(() => get().history),
      ]);
      set({
        busyAction: null,
        activeRunId: null,
        progress: null,
        runs,
        history,
        projectActivity: {
          ...get().projectActivity,
          [projectId]: { revisions: get().revisions, runs, history },
        },
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
    try {
      await desktopApi.cancelToolRun(runId);
    } catch (error: unknown) {
      set({ errorCode: errorCodeOf(error) });
    }
  },

  openRun: async (runId) => {
    set({ loading: true, errorCode: null, lastExport: null });
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
    set({ busyAction: "export", errorCode: null, lastExport: null });
    try {
      const exported = await desktopApi.exportAnalysisRun(runId, kind, language);
      const current = get().runDetails;
      const projectId = current?.run.project_id;
      const [exports, history] = await Promise.all([
        desktopApi.listRunExports(runId),
        projectId ? desktopApi.listRunHistory(projectId) : Promise.resolve(get().history),
      ]);
      set({
        busyAction: null,
        lastExport: exported,
        runDetails: current ? { ...current, exports } : current,
        history,
        projectActivity: projectId
          ? {
              ...get().projectActivity,
              [projectId]: {
                revisions: get().revisions,
                runs: get().runs,
                history,
              },
            }
          : get().projectActivity,
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
    if (import.meta.env.DEV && !desktopRuntimeAvailable()) {
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
