import { useEffect } from "react";
import { BrowserRouter, Route, Routes } from "react-router";

import { directionOf } from "@openconkit/i18n";

import { i18n } from "./i18n";
import { HomePage } from "./routes/HomePage";
import { applyTheme, systemPrefersDark, useThemeStore } from "./theme";

/**
 * Application shell: theme application, document direction (RTL/LTR) and routing.
 */
export function App() {
  const preference = useThemeStore((state) => state.preference);

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
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<HomePage />} />
      </Routes>
    </BrowserRouter>
  );
}
