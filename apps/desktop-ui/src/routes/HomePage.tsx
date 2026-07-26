import { Link } from "react-router";
import { useTranslation } from "react-i18next";

import { Icon } from "../components/Icon";
import { formatDateTime, formatNumber } from "../lib/format";
import { useWorkspaceStore } from "../state/workspace";

/** Product home: direct tool entry and persisted workbook analysis activity. */
export function HomePage() {
  const { t, i18n } = useTranslation();
  const revisions = useWorkspaceStore((state) => state.revisions);
  const runs = useWorkspaceStore((state) => state.runs);
  const recentRuns = runs
    .map((run) => ({
      run,
      revision: revisions.find((candidate) => candidate.id === run.source_revision_id),
    }))
    .sort((left, right) => right.run.started_at.localeCompare(left.run.started_at))
    .slice(0, 5);

  return (
    <main className="page-shell">
      <header className="page-header">
        <div>
          <h1>{t("home.title")}</h1>
          <p>{t("home.subtitle")}</p>
        </div>
      </header>

      <section aria-labelledby="tools-heading" className="section-block">
        <div className="section-heading">
          <h2 id="tools-heading">{t("home.tools")}</h2>
        </div>
        <Link to="/tools/boq-inspector" className="tool-row">
          <span className="tool-row-icon">
            <Icon name="clipboard" size={24} />
          </span>
          <span className="min-w-0 flex-1">
            <strong>{t("tools.boqInspector.name")}</strong>
            <span>{t("tools.boqInspector.description")}</span>
          </span>
          <span className="tool-row-action">{t("actions.openTool")}</span>
          <Icon name="chevron" size={18} className="rtl:rotate-180" />
        </Link>
      </section>

      <section aria-labelledby="runs-heading" className="section-block min-w-0">
        <div className="section-heading">
          <h2 id="runs-heading">{t("home.recentRuns")}</h2>
          <Link to="/history">{t("actions.viewHistory")}</Link>
        </div>
        {recentRuns.length === 0 ? (
          <div className="empty-panel">
            <Icon name="history" size={28} />
            <p>{t("history.empty")}</p>
            <Link to="/tools/boq-inspector" className="secondary-action">
              {t("workbooks.chooseFile")}
            </Link>
          </div>
        ) : (
          <div className="list-table" role="list">
            {recentRuns.map(({ run, revision }) => (
              <Link to="/history" key={run.id} className="list-row" role="listitem">
                <span className={`status-dot status-${run.status}`} aria-hidden="true" />
                <span className="min-w-0 flex-1">
                  <strong>{revision?.original_filename ?? t("status.notAvailable")}</strong>
                  <small>
                    {t("home.runLabel", {
                      tool: t("tools.boqInspector.name"),
                      status: t(`status.run.${run.status}`),
                    })}
                  </small>
                </span>
                <span>
                  {run.structure_diagnostics
                    ? t("home.findingContext", {
                        sheets: formatNumber(
                          run.structure_diagnostics.sheets.length,
                          i18n.language,
                        ),
                      })
                    : t("status.notAvailable")}
                </span>
                <time dateTime={run.started_at}>
                  {formatDateTime(run.started_at, i18n.language)}
                </time>
                <Icon name="chevron" size={16} className="rtl:rotate-180" />
              </Link>
            ))}
          </div>
        )}
      </section>
    </main>
  );
}
