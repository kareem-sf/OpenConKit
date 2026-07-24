import { useMemo, useState, type FormEvent } from "react";
import { useNavigate } from "react-router";
import { useTranslation } from "react-i18next";

import { Button } from "@openconkit/ui";
import type { Project } from "@openconkit/contracts";

import { Icon } from "../components/Icon";
import { formatBytes, formatDateTime, formatPercent } from "../lib/format";
import { useWorkspaceStore } from "../state/workspace";

function NewProjectDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const createProject = useWorkspaceStore((state) => state.createProject);
  const busy = useWorkspaceStore((state) => state.busyAction === "project");
  const [name, setName] = useState("");
  const [id, setId] = useState("");

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const created = await createProject(id.trim(), name.trim());
    if (created) {
      onClose();
    }
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby="new-project-title"
        className="modal-panel"
      >
        <header className="modal-header">
          <div>
            <h2 id="new-project-title">{t("projects.new")}</h2>
            <p>{t("projects.newHelp")}</p>
          </div>
          <Button
            variant="ghost"
            className="h-9 w-9 p-0"
            aria-label={t("actions.close")}
            onClick={onClose}
          >
            <Icon name="close" size={18} />
          </Button>
        </header>
        <form onSubmit={(event) => void submit(event)} className="form-stack">
          <label>
            <span>{t("projects.fields.name")}</span>
            <input
              required
              maxLength={120}
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder={t("projects.fields.namePlaceholder")}
            />
          </label>
          <label>
            <span>{t("projects.fields.id")}</span>
            <input
              required
              dir="ltr"
              maxLength={64}
              pattern="[a-z0-9]+(?:-[a-z0-9]+)*"
              value={id}
              onChange={(event) => setId(event.target.value.toLowerCase())}
              placeholder={t("projects.fields.idPlaceholder")}
            />
            <small>{t("projects.fields.idHelp")}</small>
          </label>
          <footer className="modal-actions">
            <Button variant="secondary" onClick={onClose}>
              {t("actions.cancel")}
            </Button>
            <Button type="submit" disabled={busy || !name.trim() || !id.trim()}>
              {busy ? t("status.saving") : t("projects.create")}
            </Button>
          </footer>
        </form>
      </section>
    </div>
  );
}

function ArchiveDialog({ project, onClose }: { project: Project; onClose: () => void }) {
  const { t } = useTranslation();
  const archiveProject = useWorkspaceStore((state) => state.archiveProject);
  const busy = useWorkspaceStore((state) => state.busyAction === "project");

  const archive = async () => {
    if (await archiveProject(project.id)) {
      onClose();
    }
  };

  return (
    <div className="modal-backdrop" role="presentation">
      <section
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="archive-project-title"
        className="modal-panel modal-panel-small"
      >
        <header className="modal-header">
          <div>
            <h2 id="archive-project-title">{t("projects.archiveTitle")}</h2>
            <p>{t("projects.archiveHelp", { project: project.name })}</p>
          </div>
        </header>
        <footer className="modal-actions">
          <Button variant="secondary" onClick={onClose}>
            {t("actions.cancel")}
          </Button>
          <Button variant="danger" disabled={busy} onClick={() => void archive()}>
            <Icon name="archive" size={17} />
            {t("projects.archive")}
          </Button>
        </footer>
      </section>
    </div>
  );
}

/** Project workspace with immutable source revisions and persisted run history. */
export function ProjectsPage() {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const projects = useWorkspaceStore((state) => state.projects);
  const projectActivity = useWorkspaceStore((state) => state.projectActivity);
  const selectedProjectId = useWorkspaceStore((state) => state.selectedProjectId);
  const revisions = useWorkspaceStore((state) => state.revisions);
  const runs = useWorkspaceStore((state) => state.runs);
  const selectProject = useWorkspaceStore((state) => state.selectProject);
  const chooseAndImport = useWorkspaceStore((state) => state.chooseAndImport);
  const openRun = useWorkspaceStore((state) => state.openRun);
  const busyAction = useWorkspaceStore((state) => state.busyAction);
  const [query, setQuery] = useState("");
  const [showNew, setShowNew] = useState(false);
  const [archiveCandidate, setArchiveCandidate] = useState<Project | null>(null);

  const filteredProjects = useMemo(() => {
    const probe = query.trim().toLocaleLowerCase(i18n.language);
    if (!probe) {
      return projects;
    }
    return projects.filter(
      (project) =>
        project.name.toLocaleLowerCase(i18n.language).includes(probe) || project.id.includes(probe),
    );
  }, [i18n.language, projects, query]);

  const selectedProject = projects.find((project) => project.id === selectedProjectId) ?? null;
  const latestRevision = revisions.at(-1) ?? null;
  const latestRun = runs.at(-1) ?? null;

  const openLatest = async () => {
    if (!latestRun) {
      return;
    }
    if (await openRun(latestRun.id)) {
      navigate("/tools/boq-inspector/results");
    }
  };

  return (
    <main className="project-workspace">
      <section className="project-main">
        <header className="page-header">
          <div>
            <h1>{t("projects.title")}</h1>
            <p>{t("projects.subtitle")}</p>
          </div>
          <Button onClick={() => setShowNew(true)}>
            <Icon name="plus" size={18} />
            {t("projects.new")}
          </Button>
        </header>

        <label className="search-control max-w-sm">
          <span className="sr-only">{t("projects.search")}</span>
          <Icon name="search" size={18} />
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("projects.search")}
          />
        </label>

        <div className="project-table-wrap">
          <table className="data-table project-table">
            <thead>
              <tr>
                <th>{t("projects.columns.project")}</th>
                <th>{t("projects.columns.latestSource")}</th>
                <th>{t("projects.columns.lastAnalysis")}</th>
                <th>{t("projects.columns.status")}</th>
                <th>
                  <span className="sr-only">{t("actions.more")}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {filteredProjects.map((project) => {
                const activity = projectActivity[project.id];
                const source = activity?.revisions.at(-1);
                const run = activity?.runs.at(-1);
                const selected = project.id === selectedProjectId;
                return (
                  <tr key={project.id} className={selected ? "selected-row" : undefined}>
                    <td>
                      <button
                        className="row-select-button"
                        onClick={() => void selectProject(project.id)}
                      >
                        <Icon name="folder" size={18} />
                        <span>
                          <strong>{project.name}</strong>
                          <small dir="ltr">{project.id}</small>
                        </span>
                      </button>
                    </td>
                    <td>{source?.original_filename ?? t("status.notAvailable")}</td>
                    <td>
                      {run
                        ? formatDateTime(run.started_at, i18n.language)
                        : t("status.notAvailable")}
                    </td>
                    <td>
                      {run ? (
                        <span className="status-label">
                          <span className={`status-dot status-${run.status}`} aria-hidden="true" />
                          {t(`status.run.${run.status}`)}
                        </span>
                      ) : (
                        t("projects.notAnalyzed")
                      )}
                    </td>
                    <td>
                      <Button
                        variant="ghost"
                        className="h-8 w-8 p-0"
                        aria-label={t("projects.archiveProject", {
                          project: project.name,
                        })}
                        onClick={() => setArchiveCandidate(project)}
                      >
                        <Icon name="more" size={18} />
                      </Button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {filteredProjects.length === 0 && (
            <div className="empty-panel min-h-56">
              <Icon name="folder" size={30} />
              <p>{projects.length === 0 ? t("projects.empty") : t("projects.noMatch")}</p>
            </div>
          )}
        </div>

        <section className="recent-runs-strip" aria-labelledby="project-runs-title">
          <div className="section-heading">
            <h2 id="project-runs-title">{t("projects.recentRuns")}</h2>
            <button onClick={() => navigate("/history")}>{t("actions.viewHistory")}</button>
          </div>
          {runs.length === 0 ? (
            <p className="muted-copy">{t("history.empty")}</p>
          ) : (
            <div className="compact-run-list">
              {[...runs]
                .reverse()
                .slice(0, 3)
                .map((run) => (
                  <button
                    key={run.id}
                    className="compact-run-row"
                    onClick={async () => {
                      if (await openRun(run.id)) {
                        navigate("/tools/boq-inspector/results");
                      }
                    }}
                  >
                    <time>{formatDateTime(run.started_at, i18n.language)}</time>
                    <span>{t("tools.boqInspector.name")}</span>
                    <span className="status-label">
                      <span className={`status-dot status-${run.status}`} aria-hidden="true" />
                      {t(`status.run.${run.status}`)}
                    </span>
                    <Icon name="chevron" size={16} className="rtl:rotate-180" />
                  </button>
                ))}
            </div>
          )}
        </section>
      </section>

      <aside className="project-detail" aria-label={t("projects.details")}>
        {selectedProject ? (
          <>
            <header className="detail-header">
              <div>
                <h2>{selectedProject.name}</h2>
                <p dir="ltr">{selectedProject.id}</p>
              </div>
            </header>
            <section className="detail-section">
              <h3>{t("projects.sourceReadOnly")}</h3>
              <p>{t("projects.immutableHelp")}</p>
              {latestRevision ? (
                <div className="source-record">
                  <div className="source-record-title">
                    <Icon name="file" size={22} />
                    <strong>{latestRevision.original_filename}</strong>
                    <span>{t("projects.current")}</span>
                  </div>
                  <dl>
                    <div>
                      <dt>{t("projects.sha256")}</dt>
                      <dd dir="ltr" className="hash-value">
                        {latestRevision.sha256}
                      </dd>
                    </div>
                    <div>
                      <dt>{t("projects.imported")}</dt>
                      <dd>{formatDateTime(latestRevision.imported_at, i18n.language)}</dd>
                    </div>
                    <div>
                      <dt>{t("projects.fileSize")}</dt>
                      <dd>{formatBytes(latestRevision.size_bytes, i18n.language)}</dd>
                    </div>
                  </dl>
                </div>
              ) : (
                <p className="muted-copy">{t("projects.noSources")}</p>
              )}
            </section>

            <section className="detail-section">
              <div className="section-heading">
                <h3>{t("projects.latestAnalysis")}</h3>
              </div>
              {latestRun ? (
                <div className="latest-run-summary">
                  <div>
                    <strong>{t("tools.boqInspector.name")}</strong>
                    <span className="status-label">
                      <span
                        className={`status-dot status-${latestRun.status}`}
                        aria-hidden="true"
                      />
                      {t(`status.run.${latestRun.status}`)}
                    </span>
                  </div>
                  <dl>
                    <div>
                      <dt>{t("projects.runTime")}</dt>
                      <dd>{formatDateTime(latestRun.started_at, i18n.language)}</dd>
                    </div>
                    <div>
                      <dt>{t("projects.confidence")}</dt>
                      <dd>
                        {latestRun.overall_confidence == null
                          ? t("status.notAvailable")
                          : formatPercent(latestRun.overall_confidence, i18n.language)}
                      </dd>
                    </div>
                  </dl>
                </div>
              ) : (
                <p className="muted-copy">{t("projects.notAnalyzed")}</p>
              )}
              <div className="detail-actions">
                <Button
                  disabled={!latestRun || latestRun.status !== "completed"}
                  onClick={() => void openLatest()}
                >
                  <Icon name="search" size={17} />
                  {t("projects.openLatest")}
                </Button>
                <Button
                  variant="secondary"
                  disabled={busyAction === "import"}
                  onClick={() => void chooseAndImport()}
                >
                  <Icon name="upload" size={17} />
                  {busyAction === "import" ? t("status.importing") : t("projects.importWorkbook")}
                </Button>
              </div>
            </section>

            <section className="import-panel">
              <Icon name="upload" size={30} />
              <h3>{t("projects.chooseWorkbook")}</h3>
              <p>{t("projects.copyUnchanged")}</p>
              <Button
                variant="secondary"
                disabled={busyAction === "import"}
                onClick={() => void chooseAndImport()}
              >
                {t("projects.chooseFile")}
              </Button>
            </section>
          </>
        ) : (
          <div className="empty-panel h-full">
            <Icon name="folder" size={30} />
            <p>{t("projects.selectProject")}</p>
          </div>
        )}
      </aside>

      {showNew && <NewProjectDialog onClose={() => setShowNew(false)} />}
      {archiveCandidate && (
        <ArchiveDialog project={archiveCandidate} onClose={() => setArchiveCandidate(null)} />
      )}
    </main>
  );
}
