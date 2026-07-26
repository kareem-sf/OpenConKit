import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it } from "vitest";

import { useWorkspaceStore } from "../state/workspace";
import { AppShell } from "./AppShell";

afterEach(cleanup);

describe("AppShell command error recovery", () => {
  afterEach(() => {
    useWorkspaceStore.setState({ errorCode: null });
  });

  it("preserves a current-page error until navigation, then clears it", async () => {
    useWorkspaceStore.setState({
      initialized: true,
      loading: false,
      errorCode: "BACKGROUND_TASK_FAILED",
    });

    render(
      <MemoryRouter initialEntries={["/history"]}>
        <Routes>
          <Route element={<AppShell />}>
            <Route path="/" element={<p>Home content</p>} />
            <Route path="/history" element={<p>History content</p>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByRole("alert")).toBeTruthy();
    fireEvent.click(screen.getByRole("link", { name: "Home" }));

    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(screen.getByText("Home content")).toBeTruthy();
  });
});

describe("AppShell responsive navigation", () => {
  afterEach(() => {
    useWorkspaceStore.setState({ errorCode: null });
  });

  it("opens the mobile navigation and closes it after selecting a destination", () => {
    useWorkspaceStore.setState({
      initialized: true,
      loading: false,
      errorCode: null,
    });

    render(
      <MemoryRouter initialEntries={["/history"]}>
        <Routes>
          <Route element={<AppShell />}>
            <Route path="/" element={<p>Home content</p>} />
            <Route path="/history" element={<p>History content</p>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    const toggle = screen.getByRole("button", { name: "Primary navigation" });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");

    fireEvent.click(toggle);
    expect(
      screen.getByRole("button", { name: "Close", expanded: true }).getAttribute("aria-expanded"),
    ).toBe("true");

    fireEvent.keyDown(window, { key: "Escape" });
    expect(
      screen.getByRole("button", { name: "Primary navigation" }).getAttribute("aria-expanded"),
    ).toBe("false");

    fireEvent.click(screen.getByRole("button", { name: "Primary navigation" }));
    fireEvent.click(screen.getByRole("link", { name: "Home" }));
    expect(
      screen.getByRole("button", { name: "Primary navigation" }).getAttribute("aria-expanded"),
    ).toBe("false");
  });
});
