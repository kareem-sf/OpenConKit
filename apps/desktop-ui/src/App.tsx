import { useEffect } from "react";
import { HashRouter, Route, Routes } from "react-router";

import { directionOf } from "@openconkit/i18n";

import { AppShell } from "./components/AppShell";
import { i18n } from "./i18n";
import { desktopApi, desktopRuntimeAvailable } from "./lib/ipc";
import { AboutPage } from "./routes/AboutPage";
import { BoqInspectorPage } from "./routes/BoqInspectorPage";
import { HistoryPage } from "./routes/HistoryPage";
import { HomePage } from "./routes/HomePage";
import { ProjectsPage } from "./routes/ProjectsPage";
import { SettingsPage } from "./routes/SettingsPage";
import { WelcomePage } from "./routes/WelcomePage";
import { useWorkspaceStore } from "./state/workspace";
import { applyTheme, systemPrefersDark, useThemeStore } from "./theme";

/**
 * Application shell: theme application, document direction (RTL/LTR) and routing.
 */
export function App() {
  const preference = useThemeStore((state) => state.preference);
  const setPreference = useThemeStore((state) => state.setPreference);
  const settings = useWorkspaceStore((state) => state.settings);
  const initialize = useWorkspaceStore((state) => state.initialize);
  const receiveProgress = useWorkspaceStore((state) => state.receiveProgress);
  const receiveAvailableUpdate = useWorkspaceStore((state) => state.receiveAvailableUpdate);

  useEffect(() => {
    if (
      import.meta.env.PROD ||
      new URLSearchParams(window.location.search).get("openconkit-e2e") !== "1"
    ) {
      void initialize();
      return;
    }
    const e2eReadyEvent = "openconkit:e2e-ready";
    const initializeAfterMocks = () => void initialize();
    window.addEventListener(e2eReadyEvent, initializeAfterMocks, { once: true });
    return () => window.removeEventListener(e2eReadyEvent, initializeAfterMocks);
  }, [initialize]);

  useEffect(() => {
    if (!desktopRuntimeAvailable()) {
      return;
    }
    let active = true;
    let unlisten: (() => void) | undefined;
    void desktopApi.onToolProgress(receiveProgress).then((stop) => {
      if (active) {
        unlisten = stop;
      } else {
        stop();
      }
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [receiveProgress]);

  useEffect(() => {
    if (!desktopRuntimeAvailable()) {
      return;
    }
    let active = true;
    let unlisten: (() => void) | undefined;
    void desktopApi.onUpdateAvailable(receiveAvailableUpdate).then((stop) => {
      if (active) {
        unlisten = stop;
      } else {
        stop();
      }
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [receiveAvailableUpdate]);

  useEffect(() => {
    if (!settings) {
      return;
    }
    setPreference(settings.theme);
    const language = settings.language === "system" ? navigator.language : settings.language;
    void i18n.changeLanguage(language);
  }, [setPreference, settings]);

  useEffect(() => {
    applyTheme(preference);
    if (preference !== "system") {
      return;
    }
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => applyTheme("system", media.matches || systemPrefersDark());
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [preference]);

  useEffect(() => {
    const syncDirection = () => {
      document.documentElement.dir = directionOf(i18n.language);
      document.documentElement.lang = i18n.language;
    };
    syncDirection();
    i18n.on("languageChanged", syncDirection);
    return () => {
      i18n.off("languageChanged", syncDirection);
    };
  }, []);

  return (
    <HashRouter>
      {settings && !settings.onboarding_completed ? (
        <Routes>
          <Route path="*" element={<WelcomePage />} />
        </Routes>
      ) : (
        <Routes>
          <Route element={<AppShell />}>
            <Route path="/" element={<HomePage />} />
            <Route path="/projects" element={<ProjectsPage />} />
            <Route path="/tools/boq-inspector" element={<BoqInspectorPage />} />
            <Route path="/tools/boq-inspector/results" element={<BoqInspectorPage results />} />
            <Route path="/history" element={<HistoryPage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/about" element={<AboutPage />} />
          </Route>
        </Routes>
      )}
    </HashRouter>
  );
}
