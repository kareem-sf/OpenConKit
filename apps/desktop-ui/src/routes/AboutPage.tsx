import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Icon } from "../components/Icon";
import { desktopApi, errorCodeOf } from "../lib/ipc";
import { useWorkspaceStore } from "../state/workspace";

import logoUrl from "../../../../branding/logo.svg";

/** Version, licensing and local-first product information. */
export function AboutPage() {
  const { t } = useTranslation();
  const bootstrap = useWorkspaceStore((state) => state.bootstrap);
  const [version, setVersion] = useState<string | null>(null);
  const [versionError, setVersionError] = useState<string | null>(null);

  useEffect(() => {
    void desktopApi
      .appVersion()
      .then(setVersion)
      .catch((error: unknown) => setVersionError(errorCodeOf(error)));
  }, []);

  return (
    <main className="page-shell about-page">
      <header className="about-identity">
        <img src={logoUrl} alt="" width={54} height={54} />
        <div>
          <h1>{t("app.name")}</h1>
          <p>{t("app.tagline")}</p>
          <span dir="ltr">
            {version
              ? t("about.version", { version })
              : versionError
                ? t(`errors.${versionError}`)
                : t("status.loading")}
          </span>
        </div>
      </header>

      <section className="about-grid">
        <article>
          <Icon name="check" size={24} className="text-status-success" />
          <h2>{t("about.localFirst")}</h2>
          <p>{t("about.localFirstHelp")}</p>
        </article>
        <article>
          <Icon name="file" size={24} className="text-accent-strong" />
          <h2>{t("about.readOnly")}</h2>
          <p>{t("about.readOnlyHelp")}</p>
        </article>
        <article>
          <Icon name="info" size={24} className="text-status-info" />
          <h2>{t("about.license")}</h2>
          <p>{t("about.licenseHelp")}</p>
        </article>
      </section>

      <section className="about-details">
        <h2>{t("about.installation")}</h2>
        <dl>
          <div>
            <dt>{t("about.appHome")}</dt>
            <dd dir="ltr">{bootstrap?.home_path ?? t("status.notAvailable")}</dd>
          </div>
          <div>
            <dt>{t("about.migrations")}</dt>
            <dd>
              {bootstrap?.database_migrations.length
                ? bootstrap.database_migrations.join(", ")
                : t("about.databaseCurrent")}
            </dd>
          </div>
          <div>
            <dt>{t("about.telemetry")}</dt>
            <dd>{t("about.none")}</dd>
          </div>
        </dl>
      </section>

      <p className="about-footer">{t("about.copyright")}</p>
    </main>
  );
}
