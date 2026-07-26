import { beforeEach, describe, expect, it } from "vitest";

import { previewData } from "../dev/previewData";
import { useWorkspaceStore } from "./workspace";

describe("browser preview workspace actions", () => {
  beforeEach(() => {
    const preview = previewData();
    useWorkspaceStore.setState({
      initialized: true,
      loading: false,
      busyAction: null,
      errorCode: null,
      bootstrap: preview.bootstrap,
      settings: preview.settings,
      manifests: preview.manifests,
      revisions: preview.revisions,
      runs: preview.runs,
      history: preview.history,
      runDetails: null,
      activeRunId: null,
      progress: null,
      lastExport: null,
    });
  });

  it("opens preview results without calling unavailable desktop IPC", async () => {
    const revision = previewData().revisions[0];
    expect(revision).toBeDefined();
    if (!revision) {
      return;
    }

    useWorkspaceStore.setState({ errorCode: "BACKGROUND_TASK_FAILED" });
    const details = await useWorkspaceStore.getState().runRevision(revision);

    expect(details?.run.source_revision_id).toBe(revision.id);
    expect(useWorkspaceStore.getState().runDetails?.run.id).toBe(details?.run.id);
    expect(useWorkspaceStore.getState().errorCode).toBeNull();
  });

  it("reopens preview history without calling unavailable desktop IPC", async () => {
    const run = previewData().runs[0];
    expect(run).toBeDefined();
    if (!run) {
      return;
    }

    const details = await useWorkspaceStore.getState().openRun(run.id);

    expect(details?.run.id).toBe(run.id);
    expect(useWorkspaceStore.getState().loading).toBe(false);
    expect(useWorkspaceStore.getState().errorCode).toBeNull();
  });

  it("ignores the desktop workbook picker in preview without raising an error", async () => {
    useWorkspaceStore.setState({ errorCode: "BACKGROUND_TASK_FAILED" });

    expect(await useWorkspaceStore.getState().chooseAndImport()).toBeNull();
    expect(useWorkspaceStore.getState().errorCode).toBeNull();
  });

  it("retries initialization when a previous attempt left settings unavailable", async () => {
    useWorkspaceStore.setState({
      initialized: true,
      loading: false,
      errorCode: "IPC_RESPONSE_INVALID",
      settings: null,
    });

    await useWorkspaceStore.getState().initialize();

    expect(useWorkspaceStore.getState().settings).toEqual(previewData().settings);
    expect(useWorkspaceStore.getState().errorCode).toBeNull();
  });
});
