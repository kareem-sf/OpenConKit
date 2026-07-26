import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { useWorkspaceStore } from "../state/workspace";
import { ErrorBanner } from "./ErrorBanner";

describe("ErrorBanner", () => {
  afterEach(() => {
    useWorkspaceStore.setState({ errorCode: null });
  });

  it("shows a localized command failure and dismisses it", () => {
    useWorkspaceStore.setState({ errorCode: "BACKGROUND_TASK_FAILED" });

    render(<ErrorBanner />);

    expect(screen.getByRole("alert").textContent).toContain(
      "OpenConKit could not complete that action. Try again.",
    );
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(useWorkspaceStore.getState().errorCode).toBeNull();
  });

  it("renders nothing when there is no command failure", () => {
    const { container } = render(<ErrorBanner />);
    expect(container.firstChild).toBeNull();
  });
});
