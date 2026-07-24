import { useState } from "react";
import { useNavigate } from "react-router";
import { useTranslation } from "react-i18next";

import {
  boqAiReviewSchema,
  boqInspectorOutputSchema,
  type AiAnalysis,
  type AiReviewScope,
  type BoqAiReview,
  type ExportKind,
  type Finding,
  type FindingCategory,
  type Severity,
  type SourceRevision,
} from "@openconkit/contracts";
import { Button } from "@openconkit/ui";

import { Icon } from "../components/Icon";
import {
  findingLocation,
  formatDateTime,
  formatBytes,
  formatNumber,
  formatPercent,
  supportedLocale,
} from "../lib/format";
import { desktopApi, desktopRuntimeAvailable, errorCodeOf } from "../lib/ipc";
import { useWorkspaceStore } from "../state/workspace";

const SEVERITY_ORDER: Record<Severity, number> = {
  critical: 5,
  high: 4,
  medium: 3,
  low: 2,
  info: 1,
};

function severityIcon(severity: Severity) {
  return severity === "critical" || severity === "high" ? "alert" : "info";
}

function WorkbookSelection() {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const projects = useWorkspaceStore((state) => state.projects);
  const selectedProjectId = useWorkspaceStore((state) => state.selectedProjectId);
  const revisions = useWorkspaceStore((state) => state.revisions);
  const selectProject = useWorkspaceStore((state) => state.selectProject);
  const chooseAndImport = useWorkspaceStore((state) => state.chooseAndImport);
  const runRevision = useWorkspaceStore((state) => state.runRevision);
  const busyAction = useWorkspaceStore((state) => state.busyAction);
  const progress = useWorkspaceStore((state) => state.progress);
  const cancel = useWorkspaceStore((state) => state.cancelActiveRun);
  const [selectedRevisionId, setSelectedRevisionId] = useState<string | null>(
    revisions.at(-1)?.id ?? null,
  );

  const selectedRevision =
    revisions.find((revision) => revision.id === selectedRevisionId) ?? revisions.at(-1) ?? null;

  const analyze = async (revision: SourceRevision) => {
    if (await runRevision(revision)) {
      navigate("/tools/boq-inspector/results");
    }
  };

  return (
    <main className="page-shell boq-start-page">
      <header className="page-header">
        <div>
          <h1>{t("tools.boqInspector.name")}</h1>
          <p>{t("boq.start.subtitle")}</p>
        </div>
      </header>

      <section className="analysis-setup" aria-labelledby="analysis-source-title">
        <div className="setup-main">
          <h2 id="analysis-source-title">{t("boq.start.chooseSource")}</h2>
          <p>{t("boq.start.chooseSourceHelp")}</p>

          <label className="field-control">
            <span>{t("boq.start.project")}</span>
            <select
              value={selectedProjectId ?? ""}
              onChange={(event) => {
                setSelectedRevisionId(null);
                void selectProject(event.target.value);
              }}
            >
              <option value="" disabled>
                {t("boq.start.selectProject")}
              </option>
              {projects.map((project) => (
                <option value={project.id} key={project.id}>
                  {project.name}
                </option>
              ))}
            </select>
          </label>

          {selectedProjectId && revisions.length > 0 ? (
            <div className="revision-list" role="radiogroup" aria-label={t("boq.start.sources")}>
              {[...revisions].reverse().map((revision, index) => {
                const selected = revision.id === selectedRevision?.id;
                return (
                  <label
                    key={revision.id}
                    className={`revision-row ${selected ? "revision-row-selected" : ""}`}
                  >
                    <input
                      type="radio"
                      name="source-revision"
                      value={revision.id}
                      checked={selected}
                      onChange={() => setSelectedRevisionId(revision.id)}
                    />
                    <Icon name="file" size={22} />
                    <span className="min-w-0 flex-1">
                      <strong>{revision.original_filename}</strong>
                      <small>{formatDateTime(revision.imported_at, i18n.language)}</small>
                    </span>
                    {index === 0 && <span className="subtle-label">{t("projects.current")}</span>}
                    <span dir="ltr" className="short-hash">
                      {revision.sha256.slice(0, 12)}…
                    </span>
                  </label>
                );
              })}
            </div>
          ) : (
            <div className="empty-panel min-h-52">
              <Icon name="file" size={30} />
              <h3>{t("boq.start.noWorkbook")}</h3>
              <p>{t("projects.copyUnchanged")}</p>
              <Button
                variant="secondary"
                disabled={!selectedProjectId || busyAction === "import"}
                onClick={async () => {
                  const revision = await chooseAndImport();
                  if (revision) {
                    setSelectedRevisionId(revision.id);
                  }
                }}
              >
                <Icon name="upload" size={17} />
                {t("projects.chooseFile")}
              </Button>
            </div>
          )}

          {revisions.length > 0 && (
            <div className="setup-actions">
              <Button
                variant="secondary"
                disabled={busyAction === "import"}
                onClick={async () => {
                  const revision = await chooseAndImport();
                  if (revision) {
                    setSelectedRevisionId(revision.id);
                  }
                }}
              >
                <Icon name="upload" size={17} />
                {t("projects.importWorkbook")}
              </Button>
              <Button
                disabled={!selectedRevision || busyAction === "run"}
                onClick={() => {
                  if (selectedRevision) {
                    void analyze(selectedRevision);
                  }
                }}
              >
                <Icon name="search" size={17} />
                {t("boq.start.analyze")}
              </Button>
            </div>
          )}
        </div>

        <aside className="setup-assurance">
          <Icon name="check" size={26} className="text-status-success" />
          <h2>{t("boq.start.readOnlyTitle")}</h2>
          <p>{t("boq.start.readOnlyHelp")}</p>
          <ul>
            <li>{t("boq.start.assurance.local")}</li>
            <li>{t("boq.start.assurance.unchanged")}</li>
            <li>{t("boq.start.assurance.deterministic")}</li>
          </ul>
        </aside>
      </section>

      {busyAction === "run" && progress && (
        <section className="progress-panel" aria-live="polite" aria-label={t("boq.progress.title")}>
          <div className="progress-copy">
            <span className="loading-spinner" aria-hidden="true" />
            <div>
              <strong>{t(progress.phase_key)}</strong>
              <p>{t("boq.progress.keepOpen")}</p>
            </div>
            <span>{formatPercent(progress.fraction, i18n.language)}</span>
          </div>
          <div
            className="progress-track"
            role="progressbar"
            aria-valuenow={Math.round(progress.fraction * 100)}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <span style={{ width: `${Math.max(2, progress.fraction * 100)}%` }} />
          </div>
          <Button variant="secondary" onClick={() => void cancel()}>
            {t("actions.cancelAnalysis")}
          </Button>
        </section>
      )}
    </main>
  );
}

function FindingDrawer({ finding, onClose }: { finding: Finding; onClose: () => void }) {
  const { t, i18n } = useTranslation();
  return (
    <aside className="finding-drawer" aria-label={t("boq.results.evidenceTitle")}>
      <header className="drawer-header">
        <h2>{t("boq.results.evidenceTitle")}</h2>
        <Button
          variant="ghost"
          className="h-9 w-9 p-0"
          onClick={onClose}
          aria-label={t("actions.close")}
        >
          <Icon name="close" size={18} />
        </Button>
      </header>
      <div className="drawer-scroll">
        <div className={`severity-heading severity-${finding.severity}`}>
          <Icon name={severityIcon(finding.severity)} size={18} />
          {t(`severity.${finding.severity}`)}
        </div>
        <section>
          <h3>{t("boq.results.whatWeFound")}</h3>
          <p>{t(finding.title_key, finding.title_params)}</p>
          <p>{t(finding.explanation_key, finding.explanation_params)}</p>
        </section>
        {finding.suggested_action_key && (
          <section>
            <h3>{t("boq.results.suggestedAction")}</h3>
            <p>{t(finding.suggested_action_key, finding.suggested_action_params)}</p>
          </section>
        )}
        <section className="evidence-facts">
          <h3>{t("boq.results.sourceEvidence")}</h3>
          <dl>
            <div>
              <dt>{t("boq.results.location")}</dt>
              <dd dir="ltr">{findingLocation(finding)}</dd>
            </div>
            <div>
              <dt>{t("boq.results.category")}</dt>
              <dd>{t(`category.${finding.category}`)}</dd>
            </div>
            <div>
              <dt>{t("boq.results.confidence")}</dt>
              <dd>{formatPercent(finding.confidence, i18n.language)}</dd>
            </div>
            <div>
              <dt>{t("boq.results.rule")}</dt>
              <dd dir="ltr">{finding.rule_id}</dd>
            </div>
            {finding.original_value != null && (
              <div>
                <dt>{t("boq.results.originalValue")}</dt>
                <dd dir="ltr" className="source-value">
                  {finding.original_value}
                </dd>
              </div>
            )}
            {finding.original_formula != null && (
              <div>
                <dt>{t("boq.results.originalFormula")}</dt>
                <dd dir="ltr" className="source-value">
                  {finding.original_formula}
                </dd>
              </div>
            )}
          </dl>
        </section>
        {finding.evidence.length > 0 && (
          <section>
            <h3>{t("boq.results.evidenceItems")}</h3>
            <ul className="evidence-list">
              {finding.evidence.map((evidence, index) => (
                <li key={`${evidence.sheet}-${evidence.cell ?? index}`}>
                  <span dir="ltr">
                    {evidence.sheet}
                    {evidence.cell ? `!${evidence.cell}` : ""}
                  </span>
                  <span>
                    {evidence.description_key ? t(evidence.description_key) : evidence.snippet}
                  </span>
                </li>
              ))}
            </ul>
          </section>
        )}
        <footer className="provenance-footer">
          <Icon name="check" size={17} />
          <span>{t("boq.results.deterministicOrigin")}</span>
          <code dir="ltr">{finding.rule_set_version}</code>
        </footer>
      </div>
    </aside>
  );
}

function ExportControls() {
  const { t, i18n } = useTranslation();
  const exportRun = useWorkspaceStore((state) => state.exportRun);
  const revealExport = useWorkspaceStore((state) => state.revealExport);
  const busy = useWorkspaceStore((state) => state.busyAction === "export");
  const lastExport = useWorkspaceStore((state) => state.lastExport);
  const [kind, setKind] = useState<ExportKind>("xlsx");
  const [language, setLanguage] = useState<"en" | "ar">(supportedLocale(i18n.language));

  return (
    <div className="export-controls">
      <label>
        <span className="sr-only">{t("exports.format")}</span>
        <select value={kind} onChange={(event) => setKind(event.target.value as ExportKind)}>
          <option value="xlsx">{t("exports.xlsx")}</option>
          <option value="pdf">{t("exports.pdf")}</option>
        </select>
      </label>
      <label>
        <span className="sr-only">{t("exports.language")}</span>
        <select
          value={language}
          onChange={(event) => setLanguage(event.target.value === "ar" ? "ar" : "en")}
        >
          <option value="en">{t("language.en")}</option>
          <option value="ar">{t("language.ar")}</option>
        </select>
      </label>
      <Button disabled={busy} onClick={() => void exportRun(kind, language)}>
        <Icon name="export" size={17} />
        {busy ? t("exports.generating") : t("exports.exportReport")}
      </Button>
      {lastExport && (
        <div className="export-success" role="status">
          <Icon name="check" size={16} />
          <span>
            {t("exports.created", {
              format: lastExport.kind.toUpperCase(),
            })}
          </span>
          <code dir="ltr">{lastExport.relative_path}</code>
          <Button
            variant="ghost"
            className="export-reveal"
            onClick={() => void revealExport(lastExport.id)}
          >
            <Icon name="folder" size={15} />
            {t("exports.showInFolder")}
          </Button>
        </div>
      )}
    </div>
  );
}

function AiReviewPanel({
  runId,
  analyses,
  onSelectFinding,
}: {
  runId: string;
  analyses: AiAnalysis[];
  onSelectFinding: (findingId: string) => void;
}) {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const settings = useWorkspaceStore((state) => state.settings);
  const [generated, setGenerated] = useState<AiAnalysis | null>(null);
  const [scope, setScope] = useState<AiReviewScope | null>(null);
  const [busy, setBusy] = useState(false);
  const [running, setRunning] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const language = supportedLocale(i18n.language);
  const latest =
    (generated?.language === language ? generated : null) ??
    [...analyses]
      .reverse()
      .find(
        (analysis) =>
          analysis.language === language &&
          analysis.status === "completed" &&
          analysis.grounding_status === "validated",
      ) ??
    null;
  const parsedReview = boqAiReviewSchema.safeParse(latest?.output);
  const review: BoqAiReview | null = parsedReview.success ? parsedReview.data : null;
  const enabled = settings?.privacy.ai_features_enabled ?? false;

  const prepare = async () => {
    setBusy(true);
    setErrorCode(null);
    try {
      setScope(await desktopApi.prepareAiReview(runId, language));
    } catch (error: unknown) {
      setErrorCode(errorCodeOf(error));
    } finally {
      setBusy(false);
    }
  };

  const generate = async () => {
    if (!scope) {
      return;
    }
    const acceptedScope = scope;
    setScope(null);
    setBusy(true);
    setRunning(true);
    setErrorCode(null);
    try {
      const analysis = await desktopApi.runAiReview(
        runId,
        language,
        acceptedScope.input_scope_hash,
      );
      setGenerated(analysis);
    } catch (error: unknown) {
      setErrorCode(errorCodeOf(error));
    } finally {
      setRunning(false);
      setBusy(false);
    }
  };

  const cancel = async () => {
    setCancelling(true);
    setErrorCode(null);
    try {
      await desktopApi.cancelAiReview(runId);
    } catch (error: unknown) {
      setErrorCode(errorCodeOf(error));
    } finally {
      setCancelling(false);
    }
  };

  return (
    <section className="ai-review-panel" aria-labelledby="ai-review-title">
      <div className="section-heading">
        <div>
          <span className="eyebrow">{t("boq.ai.label")}</span>
          <h2 id="ai-review-title">{t("boq.ai.title")}</h2>
          <p>{t("boq.ai.help")}</p>
        </div>
        {enabled && desktopRuntimeAvailable() ? (
          <Button disabled={busy} onClick={() => void prepare()}>
            <Icon name="sparkles" size={16} />
            {busy ? t("boq.ai.working") : review ? t("boq.ai.regenerate") : t("boq.ai.generate")}
          </Button>
        ) : (
          <Button variant="ghost" onClick={() => navigate("/settings")}>
            {t("boq.ai.enable")}
          </Button>
        )}
      </div>

      {errorCode && (
        <p role="alert" className="inline-error">
          {t(`errors.${errorCode}`, { defaultValue: t("errors.BACKGROUND_TASK_FAILED") })}
        </p>
      )}

      {review ? (
        <div className="ai-review-content">
          <p className="ai-summary">{review.summary}</p>
          {review.prioritizedRisks.length > 0 && (
            <div>
              <h3>{t("boq.ai.prioritizedRisks")}</h3>
              <div className="ai-risk-list">
                {review.prioritizedRisks.map((risk) => (
                  <article key={risk.findingIds.join(":")} className="ai-risk-card">
                    <span className={`severity-label severity-${risk.priority}`}>
                      {t(`boq.ai.priority.${risk.priority}`)}
                    </span>
                    <p>{risk.reason}</p>
                    <div className="ai-reference-list">
                      {risk.findingIds.map((findingId) => (
                        <button key={findingId} onClick={() => onSelectFinding(findingId)}>
                          {t("boq.ai.openFinding")}
                        </button>
                      ))}
                      {risk.evidenceRefs.map((reference) => (
                        <code key={reference} dir="ltr">
                          {reference}
                        </code>
                      ))}
                    </div>
                  </article>
                ))}
              </div>
            </div>
          )}
          <AiTextList title={t("boq.ai.recommendations")} values={review.recommendations} />
          <AiTextList title={t("boq.ai.rfiSuggestions")} values={review.rfiSuggestions} />
          <AiTextList title={t("boq.ai.limitations")} values={review.limitations} />
          <AiTextList title={t("boq.ai.assumptions")} values={review.assumptions} />
          {latest && (
            <p className="ai-audit-line" dir="ltr">
              {latest.model} · Codex {latest.codex_version} · {latest.input_scope_hash.slice(0, 12)}
            </p>
          )}
        </div>
      ) : (
        <p className="muted-copy">{t("boq.ai.empty")}</p>
      )}

      {running && (
        <Button variant="ghost" disabled={cancelling} onClick={() => void cancel()}>
          {cancelling ? t("boq.ai.cancelling") : t("boq.ai.cancel")}
        </Button>
      )}

      {scope && (
        <div className="modal-backdrop" role="presentation">
          <section
            className="modal-panel modal-panel-small"
            role="dialog"
            aria-modal="true"
            aria-labelledby="ai-consent-title"
          >
            <div className="modal-header">
              <div>
                <h2 id="ai-consent-title">{t("boq.ai.consentTitle")}</h2>
                <p>{t("boq.ai.consentHelp")}</p>
              </div>
              <Button
                variant="ghost"
                className="h-8 w-8 p-0"
                aria-label={t("actions.close")}
                onClick={() => setScope(null)}
              >
                <Icon name="close" size={16} />
              </Button>
            </div>
            <dl className="ai-scope-list">
              <div>
                <dt>{t("boq.ai.rows")}</dt>
                <dd>{formatNumber(scope.source_row_count, i18n.language)}</dd>
              </div>
              <div>
                <dt>{t("boq.ai.findings")}</dt>
                <dd>{formatNumber(scope.finding_count, i18n.language)}</dd>
              </div>
              <div>
                <dt>{t("boq.ai.sourceChunks")}</dt>
                <dd>{formatNumber(scope.source_chunk_count, i18n.language)}</dd>
              </div>
              <div>
                <dt>{t("boq.ai.plannedTurns")}</dt>
                <dd>{formatNumber(scope.planned_turn_count, i18n.language)}</dd>
              </div>
              <div>
                <dt>{t("boq.ai.bytes")}</dt>
                <dd>{formatBytes(scope.transmitted_bytes, i18n.language)}</dd>
              </div>
              <div>
                <dt>{t("boq.ai.sourceHash")}</dt>
                <dd>
                  <code dir="ltr">{scope.source_sha256}</code>
                </dd>
              </div>
            </dl>
            <p>{t("boq.ai.consentStatement")}</p>
            <p className="muted-copy">{t("boq.ai.offlineReminder")}</p>
            <div className="modal-actions">
              <Button variant="ghost" onClick={() => setScope(null)}>
                {t("actions.cancel")}
              </Button>
              <Button onClick={() => void generate()}>{t("boq.ai.consentConfirm")}</Button>
            </div>
          </section>
        </div>
      )}
    </section>
  );
}

function AiTextList({ title, values }: { title: string; values: string[] }) {
  if (values.length === 0) {
    return null;
  }
  return (
    <div className="ai-text-list">
      <h3>{title}</h3>
      <ul>
        {values.map((value, index) => (
          <li key={`${index}-${value}`}>{value}</li>
        ))}
      </ul>
    </div>
  );
}

function ResultsWorkspace() {
  const { t, i18n } = useTranslation();
  const details = useWorkspaceStore((state) => state.runDetails);
  const projects = useWorkspaceStore((state) => state.projects);
  const revisions = useWorkspaceStore((state) => state.revisions);
  const [search, setSearch] = useState("");
  const [severity, setSeverity] = useState<Severity | "all">("all");
  const [category, setCategory] = useState<FindingCategory | "all">("all");
  const [sort, setSort] = useState<"severity" | "confidence" | "location">("severity");
  const [selectedFindingId, setSelectedFindingId] = useState<string | null>(
    details?.findings[0]?.id ?? null,
  );

  const parsed = boqInspectorOutputSchema.safeParse(details?.output);
  if (!details || !parsed.success) {
    return (
      <main className="page-shell">
        <div className="empty-panel min-h-[60vh]">
          <Icon name="history" size={32} />
          <h1>{t("boq.results.noRunTitle")}</h1>
          <p>{t("boq.results.noRunHelp")}</p>
        </div>
      </main>
    );
  }

  const output = parsed.data;
  const project = projects.find((candidate) => candidate.id === details.run.project_id);
  const revision = revisions.find((candidate) => candidate.id === details.run.source_revision_id);
  const highCount = output.findings.filter(
    (finding) => finding.severity === "critical" || finding.severity === "high",
  ).length;
  const categories = [...new Set(output.findings.map((finding) => finding.category))].sort();
  const filtered = output.findings
    .filter((finding) => {
      if (severity !== "all" && finding.severity !== severity) {
        return false;
      }
      if (category !== "all" && finding.category !== category) {
        return false;
      }
      const probe = search.trim().toLocaleLowerCase(i18n.language);
      if (!probe) {
        return true;
      }
      const title = t(finding.title_key, finding.title_params);
      return (
        title.toLocaleLowerCase(i18n.language).includes(probe) ||
        findingLocation(finding).toLocaleLowerCase(i18n.language).includes(probe) ||
        finding.rule_id.toLocaleLowerCase(i18n.language).includes(probe)
      );
    })
    .sort((left, right) => {
      if (sort === "confidence") {
        return right.confidence - left.confidence;
      }
      if (sort === "location") {
        return findingLocation(left).localeCompare(findingLocation(right));
      }
      return SEVERITY_ORDER[right.severity] - SEVERITY_ORDER[left.severity];
    });
  const selectedFinding =
    output.findings.find((finding) => finding.id === selectedFindingId) ?? null;
  const pareto = output.summary.pareto[0] ?? null;
  const concentrationWidth = pareto
    ? Math.max(
        2,
        Math.min(100, (pareto.top_item_count / Math.max(1, pareto.total_item_count)) * 100),
      )
    : 0;

  return (
    <main className={`results-workspace ${selectedFinding ? "drawer-open" : ""}`}>
      <section className="results-main">
        <div className="context-bar">
          <span>{t("projects.title")}</span>
          <strong>{project?.name ?? details.run.project_id}</strong>
          <span className="context-divider" aria-hidden="true" />
          <span>{t("projects.sourceReadOnly")}</span>
          <strong>{revision?.original_filename ?? details.run.source_revision_id}</strong>
        </div>
        <header className="results-header">
          <div>
            <h1>{t("tools.boqInspector.name")}</h1>
            <p>
              {t("boq.results.completedAt", {
                date: formatDateTime(
                  details.run.finished_at ?? details.run.started_at,
                  i18n.language,
                ),
              })}
            </p>
          </div>
          <ExportControls />
        </header>

        <section className="metric-band" aria-label={t("boq.results.overview")}>
          <div>
            <strong className="text-status-error">
              {formatNumber(output.summary.finding_count, i18n.language)}
            </strong>
            <span>{t("boq.results.findings")}</span>
          </div>
          <div>
            <strong className="text-status-error">{formatNumber(highCount, i18n.language)}</strong>
            <span>{t("boq.results.highPriority")}</span>
          </div>
          <div>
            <strong>{formatNumber(output.summary.item_rows, i18n.language)}</strong>
            <span>{t("boq.results.itemRows")}</span>
          </div>
          <div>
            <strong className="text-status-success">
              {formatPercent(output.diagnostics.interpretation_confidence, i18n.language)}
            </strong>
            <span>{t("boq.results.structureConfidence")}</span>
          </div>
        </section>

        <section className="concentration-band" aria-labelledby="concentration-title">
          <div className="section-heading">
            <h2 id="concentration-title">{t("boq.results.concentration")}</h2>
          </div>
          {pareto ? (
            <div className="concentration-content">
              <div
                className="concentration-graphic"
                role="img"
                aria-label={t("boq.results.paretoAria")}
              >
                <div className="concentration-bar">
                  <span style={{ width: `${concentrationWidth}%` }} />
                </div>
                <div className="concentration-axis">
                  <span>{formatNumber(pareto.top_item_count, i18n.language)}</span>
                  <span>{formatNumber(pareto.total_item_count, i18n.language)}</span>
                </div>
              </div>
              <p>
                <strong>{formatNumber(pareto.top_item_count, i18n.language)}</strong>{" "}
                {t("boq.results.paretoStatement", {
                  total: formatNumber(pareto.total_item_count, i18n.language),
                  share: pareto.cumulative_share_percent,
                })}
              </p>
            </div>
          ) : (
            <p className="muted-copy">{t("boq.results.noPareto")}</p>
          )}
        </section>

        <AiReviewPanel
          runId={details.run.id}
          analyses={details.ai_analyses}
          onSelectFinding={setSelectedFindingId}
        />

        <section className="findings-region" aria-labelledby="findings-title">
          <div className="findings-toolbar">
            <label className="search-control">
              <span className="sr-only">{t("boq.results.search")}</span>
              <Icon name="search" size={17} />
              <input
                type="search"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder={t("boq.results.search")}
              />
            </label>
            <label className="select-control">
              <span className="sr-only">{t("boq.results.severityFilter")}</span>
              <select
                value={severity}
                onChange={(event) => setSeverity(event.target.value as Severity | "all")}
              >
                <option value="all">{t("boq.results.allSeverities")}</option>
                {(["critical", "high", "medium", "low", "info"] as const).map((value) => (
                  <option value={value} key={value}>
                    {t(`severity.${value}`)}
                  </option>
                ))}
              </select>
            </label>
            <label className="select-control">
              <span className="sr-only">{t("boq.results.categoryFilter")}</span>
              <select
                value={category}
                onChange={(event) => setCategory(event.target.value as FindingCategory | "all")}
              >
                <option value="all">{t("boq.results.allCategories")}</option>
                {categories.map((value) => (
                  <option value={value} key={value}>
                    {t(`category.${value}`)}
                  </option>
                ))}
              </select>
            </label>
            <label className="select-control">
              <span className="sr-only">{t("boq.results.sort")}</span>
              <Icon name="filter" size={16} />
              <select value={sort} onChange={(event) => setSort(event.target.value as typeof sort)}>
                <option value="severity">{t("boq.results.sortSeverity")}</option>
                <option value="confidence">{t("boq.results.sortConfidence")}</option>
                <option value="location">{t("boq.results.sortLocation")}</option>
              </select>
            </label>
            <span className="toolbar-count">
              {t("boq.results.filteredCount", {
                count: formatNumber(filtered.length, i18n.language),
              })}
            </span>
          </div>
          <div className="findings-table-wrap">
            <table className="data-table findings-table">
              <thead>
                <tr>
                  <th id="findings-title">{t("boq.results.severity")}</th>
                  <th>{t("boq.results.finding")}</th>
                  <th>{t("boq.results.location")}</th>
                  <th>{t("boq.results.confidence")}</th>
                  <th>{t("boq.results.category")}</th>
                  <th>
                    <span className="sr-only">{t("actions.open")}</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {filtered.slice(0, 100).map((finding) => {
                  const selected = finding.id === selectedFinding?.id;
                  return (
                    <tr key={finding.id} className={selected ? "selected-row" : undefined}>
                      <td>
                        <span className={`severity-label severity-${finding.severity}`}>
                          <Icon name={severityIcon(finding.severity)} size={16} />
                          {t(`severity.${finding.severity}`)}
                        </span>
                      </td>
                      <td>
                        <button
                          className="finding-open-button"
                          onClick={() => setSelectedFindingId(finding.id)}
                        >
                          {t(finding.title_key, finding.title_params)}
                        </button>
                      </td>
                      <td dir="ltr">{findingLocation(finding)}</td>
                      <td>{formatPercent(finding.confidence, i18n.language)}</td>
                      <td>{t(`category.${finding.category}`)}</td>
                      <td>
                        <Button
                          variant="ghost"
                          className="h-8 w-8 p-0"
                          aria-label={t("boq.results.openEvidence")}
                          onClick={() => setSelectedFindingId(finding.id)}
                        >
                          <Icon name="chevron" size={16} className="rtl:rotate-180" />
                        </Button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            {filtered.length === 0 && (
              <div className="empty-panel min-h-48">
                <Icon name="search" size={28} />
                <p>{t("boq.results.noMatchingFindings")}</p>
              </div>
            )}
          </div>
        </section>

        <footer className="results-footer">
          <span>
            <Icon name="check" size={16} className="text-status-success" />
            {t("boq.results.deterministicFooter")}
          </span>
          <span dir="ltr">
            {t("boq.results.ruleSet")}: {details.run.rule_set_version}
          </span>
          <span className="ai-affordance">
            <Icon name="sparkles" size={16} />
            {t("boq.ai.footer")}
          </span>
        </footer>
      </section>
      {selectedFinding && (
        <FindingDrawer finding={selectedFinding} onClose={() => setSelectedFindingId(null)} />
      )}
    </main>
  );
}

export function BoqInspectorPage({ results = false }: { results?: boolean }) {
  return results ? <ResultsWorkspace /> : <WorkbookSelection />;
}
