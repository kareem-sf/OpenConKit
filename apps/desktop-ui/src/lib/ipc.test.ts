import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  isTauri: () => true,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

import { desktopApi } from "./ipc";

describe("desktop IPC response validation", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("reports an incompatible response contract instead of a generic task failure", async () => {
    invokeMock.mockResolvedValue({
      schema_version: 1,
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
    });

    await expect(desktopApi.getSettings()).rejects.toMatchObject({
      name: "IpcCommandError",
      code: "IPC_RESPONSE_INVALID",
    });
  });

  it("preserves stable backend error codes", async () => {
    invokeMock.mockRejectedValue({ code: "STORAGE_FAILED" });

    await expect(desktopApi.getSettings()).rejects.toMatchObject({
      name: "IpcCommandError",
      code: "STORAGE_FAILED",
    });
  });
});
