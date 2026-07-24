import { Link } from "react-router";
import { useTranslation } from "react-i18next";

import { Icon } from "../components/Icon";
import { formatDateTime, formatNumber } from "../lib/format";
import { useWorkspaceStore } from "../state/workspace";

/** Product home: real tool entry, projects and persisted run activity. */
export function HomePage() {
  const { t, i18n } = useTranslation();
  const projects = useWorkspaceStore((state) => state.projects);
  const projectActivity = useWorkspaceStore((state) => state.projectActivity);
  const selectedProjectId = useWorkspaceStore((state) => state.selectedProjectId);
  const selectedProject = projects.find((project) => project.id === selectedProjectId);
  const recentRuns = projects
    .flatMap((project) =>
      (projectActivity[project.id]?.runs ?? []).map((run) => ({
        run,
        project,
      })),
    )
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
          <span className="text-sm font-medium text-accent-strong">{t("actions.openTool")}</span>
          <Icon name="chevron" size={18} className="rtl:rotate-180" />
        </Link>
      </section>

      <div className="dashboard-split">
        <section aria-labelledby="projects-heading" className="section-block min-w-0">
          <div className="section-heading">
            <h2 id="projects-heading">{t("home.recentProjects")}</h2>
            <Link to="/projects">{t("actions.viewAll")}</Link>
          </div>
          {projects.length === 0 ? (
            <div className="empty-panel">
              <Icon name="folder" size={28} />
              <p>{t("projects.empty")}</p>
              <Link to="/projects" className="secondary-action">
                {t("projects.new")}
              </Link>
            </div>
          ) : (
            <div className="list-table" role="list">
              {projects.slice(0, 5).map((project) => (
                <Link to="/projects" key={project.id} className="list-row" role="listitem">
                  <Icon name="folder" size={18} />
                  <span className="min-w-0 flex-1">
                    <strong className="truncate">{project.name}</strong>
                    <small dir="ltr">{project.id}</small>
                  </span>
                  <time dateTime={project.updated_at}>
                    {formatDateTime(project.updated_at, i18n.language)}
                  </time>
                  <Icon name="chevron" size={16} className="rtl:rotate-180" />
                </Link>
              ))}
            </div>
          )}
        </section>

        <section aria-labelledby="runs-heading" className="section-block min-w-0">
          <div className="section-heading">
            <h2 id="runs-heading">{t("home.recentRuns")}</h2>
            <Link to="/history">{t("actions.viewHistory")}</Link>
          </div>
          {recentRuns.length === 0 ? (
            <div className="empty-panel">
              <Icon name="history" size={28} />
              <p>
                {selectedProject
                  ? t("history.noneForProject", { project: selectedProject.name })
                  : t("history.empty")}
              </p>
            </div>
          ) : (
            <div className="list-table" role="list">
              {recentRuns.map(({ run, project }) => (
                <Link to="/history" key={run.id} className="list-row" role="listitem">
                  <span className={`status-dot status-${run.status}`} aria-hidden="true" />
                  <span className="min-w-0 flex-1">
                    <strong>{project.name}</strong>
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
                </Link>
              ))}
            </div>
          )}
        </section>
      </div>
    </main>
  );
}
