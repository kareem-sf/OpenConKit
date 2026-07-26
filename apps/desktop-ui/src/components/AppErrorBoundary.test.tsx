import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AppErrorBoundary } from "./AppErrorBoundary";

function BrokenScreen(): never {
  throw new Error("synthetic render failure");
}

describe("AppErrorBoundary", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders an actionable fallback instead of a blank window", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);

    render(
      <AppErrorBoundary>
        <BrokenScreen />
      </AppErrorBoundary>,
    );

    expect(screen.getByRole("heading", { name: "OpenConKit needs to restart" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Restart OpenConKit" })).toBeDefined();
  });
});
