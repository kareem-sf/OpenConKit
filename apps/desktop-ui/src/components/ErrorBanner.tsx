import { useTranslation } from "react-i18next";

import { Button } from "@openconkit/ui";

import { useWorkspaceStore } from "../state/workspace";
import { Icon } from "./Icon";

/** Localized, privacy-safe global command failure notice. */
export function ErrorBanner() {
  const { t } = useTranslation();
  const errorCode = useWorkspaceStore((state) => state.errorCode);
  const dismiss = useWorkspaceStore((state) => state.dismissError);

  if (!errorCode) {
    return null;
  }

  return (
    <div
      role="alert"
      className="mx-6 mt-4 flex items-center gap-3 border border-status-error bg-status-error-subtle px-4 py-3 text-sm text-content-primary"
    >
      <Icon name="alert" className="shrink-0 text-status-error" size={18} />
      <p className="min-w-0 flex-1">
        {t(`errors.${errorCode}`, {
          defaultValue: t("errors.BACKGROUND_TASK_FAILED"),
        })}
      </p>
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
