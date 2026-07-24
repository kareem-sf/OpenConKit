import { create } from "zustand";

/** User theme preference; `system` follows the OS setting. */
export type ThemePreference = "system" | "light" | "dark";

/** Concrete scheme applied to the document. */
export type ResolvedTheme = "light" | "dark";

interface ThemeState {
  preference: ThemePreference;
  setPreference: (preference: ThemePreference) => void;
}

/**
 * In-memory view of the canonical backend setting. Persistence lives only
 * under the app home through `openconkit-storage`; the WebView never creates
 * a second settings source in localStorage.
 */
export const useThemeStore = create<ThemeState>((set) => ({
  preference: "system",
  setPreference: (preference) => set({ preference }),
}));

/** Resolve a preference against the OS scheme. */
export function resolveTheme(preference: ThemePreference, systemDark: boolean): ResolvedTheme {
  if (preference === "system") {
    return systemDark ? "dark" : "light";
  }
  return preference;
}

/** Whether the OS currently prefers a dark scheme. */
export function systemPrefersDark(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
  );
}

/** Apply a theme preference to the document (`data-theme` attribute). */
export function applyTheme(
  preference: ThemePreference,
  systemDark = systemPrefersDark(),
): ResolvedTheme {
  const resolved = resolveTheme(preference, systemDark);
  document.documentElement.dataset.theme = resolved;
  return resolved;
}
