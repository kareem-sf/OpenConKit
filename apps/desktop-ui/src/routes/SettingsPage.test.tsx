import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { previewData } from "../dev/previewData";
import { useWorkspaceStore } from "../state/workspace";
import { SettingsPage } from "./SettingsPage";

describe("SettingsPage reset", () => {
  beforeEach(() => {
    const preview = previewData();
    useWorkspaceStore.setState({
      busyAction: null,
      errorCode: null,
      settings: preview.settings,
      bootstrap: preview.bootstrap,
      manifests: preview.manifests,
      revisions: preview.revisions,
      runs: preview.runs,
      history: preview.history,
    });
  });

  afterEach(() => {
    window.location.hash = "";
  });

  it("requires typed confirmation and returns the preview to onboarding", async () => {
    render(<SettingsPage />);
    fireEvent.click(screen.getByRole("button", { name: "Reset OpenConKit" }));

    const dialog = screen.getByRole("alertdialog");
    const confirm = screen.getByTestId("reset-openconkit-confirm");
    expect(confirm.hasAttribute("disabled")).toBe(true);

    fireEvent.change(screen.getByRole("textbox", { name: "Type RESET to confirm" }), {
      target: { value: "RESET" },
    });
    expect(confirm.hasAttribute("disabled")).toBe(false);
    fireEvent.click(confirm);

    await waitFor(() => {
      expect(useWorkspaceStore.getState().settings?.onboarding_completed).toBe(false);
    });
    expect(useWorkspaceStore.getState().revisions).toEqual([]);
    expect(useWorkspaceStore.getState().runs).toEqual([]);
    expect(useWorkspaceStore.getState().history).toEqual([]);
    expect(dialog.isConnected).toBe(false);
  });
});
