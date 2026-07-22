import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { Button } from "@openconkit/ui";

import { applyTheme, resolveTheme, useThemeStore } from "../theme";

describe("theme store", () => {
  afterEach(() => {
    useThemeStore.getState().setPreference("system");
    document.documentElement.removeAttribute("data-theme");
  });

  it("defaults to the system preference", () => {
    expect(useThemeStore.getState().preference).toBe("system");
  });

  it("persists an explicit preference", () => {
    useThemeStore.getState().setPreference("dark");
    expect(useThemeStore.getState().preference).toBe("dark");
  });

  it("resolves system against the OS scheme", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });

  it("applies the resolved theme to the document", () => {
    expect(applyTheme("dark", false)).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(applyTheme("system", false)).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });
});

describe("Button primitive", () => {
  it("renders an accessible button with a label", () => {
    render(<Button>Run checks</Button>);
    const button = screen.getByRole("button", { name: "Run checks" });
    expect(button).toBeDefined();
    expect(button.getAttribute("type")).toBe("button");
  });
});
