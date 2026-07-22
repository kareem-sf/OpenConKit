import { useTranslation } from "react-i18next";

import { Button } from "@openconkit/ui";

import logoUrl from "../../../../branding/logo.svg";

/**
 * Placeholder home route: brand mark, localized tagline and tool entry point.
 */
export function HomePage() {
  const { t } = useTranslation();

  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-6 bg-surface-base px-8 text-center">
      <img src={logoUrl} alt={t("app.name")} className="h-24 w-24" width={96} height={96} />
      <h1 className="text-3xl font-semibold text-content-primary">{t("app.name")}</h1>
      <p className="max-w-md text-lg text-content-secondary">{t("app.tagline")}</p>
      <Button variant="primary">{t("tools.boqInspector.name")}</Button>
    </main>
  );
}
