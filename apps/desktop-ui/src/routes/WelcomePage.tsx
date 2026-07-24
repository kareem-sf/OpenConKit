import { useTranslation } from "react-i18next";

import type { SettingsPatch } from "@openconkit/contracts";
import { Button } from "@openconkit/ui";

import logoUrl from "../../../../branding/logo.svg";

import { ErrorBanner } from "../components/ErrorBanner";
import { Icon } from "../components/Icon";
import { useWorkspaceStore } from "../state/workspace";

/** Durable first-run privacy and local-first acknowledgement. */
export function WelcomePage() {
  const { t } = useTranslation();
  const saveSettings = useWorkspaceStore((state) => state.saveSettings);
  const busy = useWorkspaceStore((state) => state.busyAction === "settings");
  const home = useWorkspaceStore((state) => state.bootstrap?.home_path);

  const complete = async () => {
    const patch: SettingsPatch = {
      onboarding_completed: true,
      language: null,
      theme: null,
      update_channel: null,
      tolerances: null,
      privacy: null,
      advanced: null,
      last_successful_update_check: null,
    };
    await saveSettings(patch);
  };

  return (
    <main className="welcome-page">
      <div className="welcome-error">
        <ErrorBanner />
      </div>
      <section className="welcome-card" aria-labelledby="welcome-title">
        <header className="welcome-header">
          <img src={logoUrl} alt="" width={52} height={52} />
          <div>
            <span className="welcome-eyebrow">{t("app.name")}</span>
            <h1 id="welcome-title">{t("welcome.title")}</h1>
            <p>{t("welcome.subtitle")}</p>
          </div>
        </header>

        <div className="welcome-principles">
          <article>
            <span className="welcome-icon">
              <Icon name="folder" size={22} />
            </span>
            <div>
              <h2>{t("welcome.localTitle")}</h2>
              <p>{t("welcome.localHelp")}</p>
              {home && <code dir="ltr">{home}</code>}
            </div>
          </article>
          <article>
            <span className="welcome-icon">
              <Icon name="file" size={22} />
            </span>
            <div>
              <h2>{t("welcome.sourceTitle")}</h2>
              <p>{t("welcome.sourceHelp")}</p>
            </div>
          </article>
          <article>
            <span className="welcome-icon">
              <Icon name="sparkles" size={22} />
            </span>
            <div>
              <h2>{t("welcome.aiTitle")}</h2>
              <p>{t("welcome.aiHelp")}</p>
            </div>
          </article>
        </div>

        <footer className="welcome-footer">
          <p>
            <Icon name="shield" size={17} />
            {t("welcome.noTelemetry")}
          </p>
          <Button data-testid="welcome-continue" disabled={busy} onClick={() => void complete()}>
            {busy ? t("status.saving") : t("welcome.continue")}
            <Icon name="chevron" size={17} className="rtl:rotate-180" />
          </Button>
        </footer>
      </section>
    </main>
  );
}
