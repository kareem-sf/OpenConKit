import { NavLink, Outlet } from "react-router";
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
  { to: "/projects", labelKey: "nav.projects", icon: "folder" },
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
  const initialized = useWorkspaceStore((state) => state.initialized);
  const loading = useWorkspaceStore((state) => state.loading);

  return (
    <div className="app-frame">
      <aside className="app-sidebar" aria-label={t("nav.primary")}>
        <div className="app-brand">
          <img src={logoUrl} alt="" className="h-9 w-9" width={36} height={36} />
          <span>{t("app.name")}</span>
        </div>
        <nav className="mt-8 flex flex-1 flex-col gap-1 px-2">
          {NAVIGATION.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.end}
              className={({ isActive }) => `app-nav-item ${isActive ? "app-nav-item-active" : ""}`}
            >
              <Icon name={item.icon} size={20} />
              <span>{t(item.labelKey)}</span>
            </NavLink>
          ))}
        </nav>
        <NavLink to="/about" className="app-nav-item mx-2 mb-3">
          <Icon name="info" size={20} />
          <span>{t("nav.about")}</span>
        </NavLink>
      </aside>
      <div className="min-w-0 flex-1 bg-surface-base">
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
