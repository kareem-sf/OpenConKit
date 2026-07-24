// @vitest-environment jsdom

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Button } from "./Button";

describe("Button", () => {
  it("defaults to a non-submitting accessible button", () => {
    render(<Button>Run checks</Button>);
    const button = screen.getByRole("button", { name: "Run checks" });
    expect(button.getAttribute("type")).toBe("button");
    expect(button.className).toContain("bg-accent");
  });

  it("forwards native behavior and secondary styling", () => {
    const onClick = vi.fn();
    render(
      <Button variant="secondary" type="submit" onClick={onClick}>
        Save
      </Button>,
    );
    const button = screen.getByRole("button", { name: "Save" });
    fireEvent.click(button);
    expect(onClick).toHaveBeenCalledOnce();
    expect(button.getAttribute("type")).toBe("submit");
    expect(button.className).toContain("bg-surface-muted");
  });
});
