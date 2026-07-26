import { useMemo, useState } from "react";
import { useNavigate } from "react-router";
import { useTranslation } from "react-i18next";

import { Button } from "@openconkit/ui";

import { Icon } from "../components/Icon";
import { formatDateTime, formatNumber, formatPercent } from "../lib/format";
import { useWorkspaceStore } from "../state/workspace";

/** Persisted analysis history across every imported workbook. */
export function HistoryPage() {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const history = useWorkspaceStore((state) => state.history);
  const revisions = useWorkspaceStore((state) => state.revisions);
  const openRun = useWorkspaceStore((state) => state.openRun);
  const [statusFilter, setStatusFilter] = useState("all");

  const rows = useMemo(
    () =>
      history
        .map((entry) => ({
          entry,
          run: entry.run,
          revision: revisions.find((candidate) => candidate.id === entry.run.source_revision_id),
        }))
        .filter(({ run }) => statusFilter === "all" || run.status === statusFilter)
        .sort((left, right) => right.run.started_at.localeCompare(left.run.started_at)),
    [history, revisions, statusFilter],
  );

  const showResults = async (runId: string) => {
    if (await openRun(runId)) {
      navigate("/tools/boq-inspector/results");
    }
  };

  return (
    <main className="page-shell history-page">
      <header className="page-header">
        <div>
          <h1>{t("history.title")}</h1>
          <p>{t("history.subtitle")}</p>
        </div>
      </header>

      <div className="history-toolbar">
        <label className="select-control">
          <span>{t("history.statusFilter")}</span>
          <select value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}>
            <option value="all">{t("history.allStatuses")}</option>
            {(["completed", "failed", "cancelled", "running", "pending"] as const).map((status) => (
              <option value={status} key={status}>
                {t(`status.run.${status}`)}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="history-table-wrap">
        <table className="data-table history-table">
          <thead>
            <tr>
              <th>{t("history.columns.when")}</th>
              <th>{t("history.columns.source")}</th>
              <th>{t("history.columns.tool")}</th>
              <th>{t("history.columns.status")}</th>
              <th>{t("history.columns.confidence")}</th>
              <th>{t("history.columns.findings")}</th>
              <th>{t("history.columns.exports")}</th>
              <th>{t("history.columns.ai")}</th>
              <th>{t("history.columns.version")}</th>
              <th>
                <span className="sr-only">{t("actions.open")}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.map(({ entry, run, revision }) => (
              <tr key={run.id}>
                <td data-label={t("history.columns.when")}>
                  <time dateTime={run.started_at}>
                    {formatDateTime(run.started_at, i18n.language)}
                  </time>
                </td>
                <td data-label={t("history.columns.source")}>
                  <strong>{revision?.original_filename ?? t("status.notAvailable")}</strong>
                  <code className="history-source-hash" dir="ltr">
                    {entry.source_sha256}
                  </code>
                </td>
                <td data-label={t("history.columns.tool")}>{t("tools.boqInspector.name")}</td>
                <td data-label={t("history.columns.status")}>
                  <span className="status-label">
                    <span className={`status-dot status-${run.status}`} aria-hidden="true" />
                    {t(`status.run.${run.status}`)}
                  </span>
                </td>
                <td data-label={t("history.columns.confidence")}>
                  {run.overall_confidence == null
                    ? t("status.notAvailable")
                    : formatPercent(run.overall_confidence, i18n.language)}
                </td>
                <td data-label={t("history.columns.findings")}>
                  {formatNumber(entry.finding_count, i18n.language)}
                </td>
                <td data-label={t("history.columns.exports")}>
                  {formatNumber(entry.export_count, i18n.language)}
                </td>
                <td data-label={t("history.columns.ai")}>
                  {entry.latest_ai_status
                    ? t(`history.ai.${entry.latest_ai_status}`)
                    : t("history.ai.none")}
                </td>
                <td data-label={t("history.columns.version")} dir="ltr">
                  {run.rule_set_version}
                </td>
                <td data-label={t("actions.open")}>
                  <Button
                    variant="ghost"
                    className="h-8 w-8 p-0"
                    disabled={run.status !== "completed"}
                    aria-label={t("history.openRun")}
                    onClick={() => void showResults(run.id)}
                  >
                    <Icon name="chevron" size={16} className="rtl:rotate-180" />
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {rows.length === 0 && (
          <div className="empty-panel min-h-72">
            <Icon name="history" size={32} />
            <h2>{t("history.empty")}</h2>
            <p>{t("history.emptyHelp")}</p>
          </div>
        )}
      </div>
    </main>
  );
}
