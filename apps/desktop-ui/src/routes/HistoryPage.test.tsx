import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { MemoryRouter } from "react-router";

import { previewData } from "../dev/previewData";
import { useWorkspaceStore } from "../state/workspace";
import { HistoryPage } from "./HistoryPage";

describe("HistoryPage", () => {
  beforeEach(() => {
    const preview = previewData();
    useWorkspaceStore.setState({
      loading: false,
      revisions: preview.revisions,
      runs: preview.runs,
      history: preview.history,
      runDetails: null,
    });
  });

  it("shows workbook-based history without project controls or columns", () => {
    render(
      <MemoryRouter>
        <HistoryPage />
      </MemoryRouter>,
    );

    expect(screen.getByRole("heading", { name: "Analysis history" })).toBeTruthy();
    expect(screen.getByText("Priced BOQ Rev 03.xlsx")).toBeTruthy();
    expect(screen.queryByText("Project")).toBeNull();
    expect(screen.queryByRole("combobox", { name: "Project" })).toBeNull();
    expect(screen.getAllByRole("combobox")).toHaveLength(1);
  });
});
