import { beforeEach, describe, expect, it, vi } from "vitest";

const { apiMocks } = vi.hoisted(() => ({
  apiMocks: {
    bootstrapStatus: vi.fn(),
    getSettings: vi.fn(),
    listToolManifests: vi.fn(),
    listStorageGroups: vi.fn(),
  },
}));

vi.mock("../lib/ipc", () => ({
  desktopApi: apiMocks,
  desktopRuntimeAvailable: () => true,
  errorCodeOf: (error: unknown) => {
    if (
      typeof error === "object" &&
      error !== null &&
      "code" in error &&
      typeof error.code === "string"
    ) {
      return error.code;
    }
    return "BACKGROUND_TASK_FAILED";
  },
}));

import { previewData } from "../dev/previewData";
import { useWorkspaceStore } from "./workspace";

describe("desktop workspace initialization", () => {
  beforeEach(() => {
    const preview = previewData();
    apiMocks.bootstrapStatus.mockReset().mockResolvedValue(preview.bootstrap);
    apiMocks.getSettings.mockReset().mockResolvedValue(preview.settings);
    apiMocks.listToolManifests.mockReset().mockResolvedValue(preview.manifests);
    apiMocks.listStorageGroups.mockReset().mockResolvedValue([]);
    useWorkspaceStore.setState({
      initialized: false,
      loading: false,
      errorCode: null,
      bootstrap: null,
      settings: null,
      manifests: [],
      revisions: [],
      runs: [],
      history: [],
    });
  });

  it("keeps fulfilled startup data when another local source fails", async () => {
    apiMocks.listToolManifests.mockRejectedValue({ code: "TOOL_REGISTRY" });

    await useWorkspaceStore.getState().initialize();

    expect(useWorkspaceStore.getState()).toMatchObject({
      initialized: true,
      loading: false,
      errorCode: "TOOL_REGISTRY",
      bootstrap: previewData().bootstrap,
      settings: previewData().settings,
      manifests: [],
      revisions: [],
      runs: [],
      history: [],
    });
  });
});
