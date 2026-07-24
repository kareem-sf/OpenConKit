import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it } from "vitest";

import { useWorkspaceStore } from "../state/workspace";
import { UpdateBanner } from "./UpdateBanner";

describe("UpdateBanner", () => {
  afterEach(() => {
    useWorkspaceStore.setState({ availableUpdate: null });
  });

  it("shows a validated background update and can dismiss it", () => {
    useWorkspaceStore.setState({
      availableUpdate: {
        checked_at: "2026-07-24T08:00:00Z",
        channel: "stable",
        current_version: "1.0.0",
        portable: false,
        update: {
          version: "1.1.0",
          notes: "Release notes",
          published_at: "2026-07-24T07:00:00Z",
          size_bytes: 1024,
          can_install: true,
          manual_download_url: "https://github.com/kareem-sf/OpenConKit/releases/tag/v1.1.0",
        },
      },
    });

    render(
      <MemoryRouter>
        <UpdateBanner />
      </MemoryRouter>,
    );
    expect(screen.getByRole("status").textContent).toContain("1.1.0");
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(useWorkspaceStore.getState().availableUpdate).toBeNull();
  });

  it("renders nothing when no update is available", () => {
    const { container } = render(
      <MemoryRouter>
        <UpdateBanner />
      </MemoryRouter>,
    );
    expect(container.firstChild).toBeNull();
  });
});
