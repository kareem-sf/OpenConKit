import { create } from "zustand";
import { createJSONStorage, persist, type StateStorage } from "zustand/middleware";

/** User theme preference; `system` follows the OS setting. */
export type ThemePreference = "system" | "light" | "dark";

/** Concrete scheme applied to the document. */
export type ResolvedTheme = "light" | "dark";

/** In-memory storage used where localStorage is unavailable (tests). */
const memoryStorage = new Map<string, string>();
const memoryStateStorage: StateStorage = {
  getItem: (name) => memoryStorage.get(name) ?? null,
  setItem: (name, value) => {
    memoryStorage.set(name, value);
  },
  removeItem: (name) => {
    memoryStorage.delete(name);
  },
};

/** Prefer real localStorage; fall back to memory when unavailable. */
function resolveStorage(): StateStorage {
  try {
    if (typeof window !== "undefined" && window.localStorage) {
      return window.localStorage;
    }
  } catch {
    // Access can throw in sandboxed contexts; fall through to memory.
  }
  return memoryStateStorage;
}

interface ThemeState {
  preference: ThemePreference;
  setPreference: (preference: ThemePreference) => void;
}

/**
 * Theme store. The preference is persisted to localStorage until the
 * settings backend (openconkit-storage) lands.
 */
export const useThemeStore = create<ThemeState>()(
  persist(
    (set) => ({
      preference: "system",
      setPreference: (preference) => set({ preference }),
    }),
    { name: "openconkit.theme", storage: createJSONStorage(resolveStorage) },
  ),
);

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
