import { useEffect, useRef, useState } from "react";
import { NavLink, Outlet, useLocation } from "react-router";
import { useTranslation } from "react-i18next";

import logoUrl from "../../../../branding/logo.svg";

import { useWorkspaceStore } from "../state/workspace";
import { ErrorBanner } from "./ErrorBanner";
import { Icon, type IconName } from "./Icon";
import { UpdateBanner } from "./UpdateBanner";

const NAVIGATION: ReadonlyArray<{
  to: string;
  labelKey: string;
  icon: IconName;
  end?: boolean;
}> = [
  { to: "/", labelKey: "nav.home", icon: "home", end: true },
  {
    to: "/tools/boq-inspector",
    labelKey: "tools.boqInspector.name",
    icon: "clipboard",
  },
  { to: "/history", labelKey: "nav.history", icon: "history" },
  { to: "/settings", labelKey: "nav.settings", icon: "settings" },
];

/** Persistent desktop navigation rail and content frame. */
export function AppShell() {
  const { t } = useTranslation();
  const location = useLocation();
  const [navigationOpen, setNavigationOpen] = useState(false);
  const initialized = useWorkspaceStore((state) => state.initialized);
  const loading = useWorkspaceStore((state) => state.loading);
  const dismissError = useWorkspaceStore((state) => state.dismissError);
  const previousPath = useRef(location.pathname);

  useEffect(() => {
    if (previousPath.current !== location.pathname) {
      dismissError();
      previousPath.current = location.pathname;
    }
  }, [dismissError, location.pathname]);

  useEffect(() => {
    if (!navigationOpen) {
      return;
    }
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setNavigationOpen(false);
      }
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [navigationOpen]);

  return (
    <div className="app-frame">
      <header className="mobile-app-bar">
        <button
          type="button"
          className="mobile-nav-toggle"
          aria-label={navigationOpen ? t("actions.close") : t("nav.primary")}
          aria-controls="primary-navigation"
          aria-expanded={navigationOpen}
          onClick={() => setNavigationOpen((open) => !open)}
        >
          <Icon name={navigationOpen ? "close" : "menu"} size={22} />
        </button>
        <div className="mobile-app-brand">
          <img src={logoUrl} alt="" width={24} height={24} />
          <span>{t("app.name")}</span>
        </div>
      </header>
      <button
        type="button"
        className={`mobile-nav-scrim ${navigationOpen ? "mobile-nav-scrim-visible" : ""}`}
        aria-label={t("actions.close")}
        tabIndex={navigationOpen ? 0 : -1}
        onClick={() => setNavigationOpen(false)}
      />
      <aside
        id="primary-navigation"
        className={`app-sidebar ${navigationOpen ? "app-sidebar-open" : ""}`}
        aria-label={t("nav.primary")}
      >
        <div className="app-brand">
          <img src={logoUrl} alt="" className="app-brand-logo" width={27} height={27} />
          <span>{t("app.name")}</span>
        </div>
        <nav className="app-nav">
          {NAVIGATION.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.end}
              onClick={() => setNavigationOpen(false)}
              className={({ isActive }) => `app-nav-item ${isActive ? "app-nav-item-active" : ""}`}
            >
              <Icon name={item.icon} size={20} />
              <span>{t(item.labelKey)}</span>
            </NavLink>
          ))}
        </nav>
        <NavLink
          to="/about"
          className="app-nav-item app-nav-about"
          onClick={() => setNavigationOpen(false)}
        >
          <Icon name="info" size={20} />
          <span>{t("nav.about")}</span>
        </NavLink>
      </aside>
      <div className="app-content min-w-0 flex-1 bg-surface-base">
        <ErrorBanner />
        <UpdateBanner />
        {!initialized || loading ? (
          <div className="flex min-h-[calc(100vh-5rem)] items-center justify-center">
            <div className="flex items-center gap-3 text-sm text-content-secondary" role="status">
              <span className="loading-spinner" aria-hidden="true" />
              {t("status.loading")}
            </div>
          </div>
        ) : (
          <Outlet />
        )}
      </div>
    </div>
  );
}
