import { NavLink } from "react-router";
import { useTranslation } from "react-i18next";

import { Button } from "@openconkit/ui";

import { useWorkspaceStore } from "../state/workspace";
import { Icon } from "./Icon";

/** Non-blocking notice emitted by the 24-hour background update check. */
export function UpdateBanner() {
  const { t } = useTranslation();
  const result = useWorkspaceStore((state) => state.availableUpdate);
  const dismiss = useWorkspaceStore((state) => state.dismissAvailableUpdate);

  if (!result?.update) {
    return null;
  }

  return (
    <div
      role="status"
      className="mx-6 mt-4 flex items-center gap-3 border border-status-info bg-status-info-subtle px-4 py-3 text-sm text-content-primary"
    >
      <Icon name="info" className="shrink-0 text-status-info" size={18} />
      <p className="min-w-0 flex-1">
        {t("settings.updateAvailable", { version: result.update.version })}
      </p>
      <NavLink to="/settings" className="update-banner-link" onClick={dismiss}>
        {t("settings.reviewUpdate")}
      </NavLink>
      <Button
        variant="ghost"
        className="h-8 w-8 shrink-0 p-0"
        aria-label={t("actions.dismiss")}
        onClick={dismiss}
      >
        <Icon name="close" size={16} />
      </Button>
    </div>
  );
}
