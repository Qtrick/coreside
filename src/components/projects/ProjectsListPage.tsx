import { Folder, FolderPlus } from "lucide-react";
import type { Project } from "@/types/project";
import { EMPTY_STATES, openHelpAndLearning } from "@/lib/empty-states";
import { useAppStore } from "@/stores/app-store";
import { projectIconClass } from "./project-icon";

type ProjectsListPageProps = {
  projects: Project[];
  onCreateProject: () => void;
  onOpenProject: (projectId: string) => void;
};

export function ProjectsListPage({
  projects,
  onCreateProject,
  onOpenProject,
}: ProjectsListPageProps) {
  const navigateToSettings = useAppStore((s) => s.navigateToSettings);
  const visible = projects.filter((p) => !p.archived);

  return (
    <section className="projects-list-page" aria-label="Projects">
      <header className="panel-header">
        <div>
          <h1>Projects</h1>
          <p className="panel-subtitle">
            Organize chats with shared instructions and context.
          </p>
        </div>
        <button type="button" className="btn btn-primary" onClick={onCreateProject}>
          <FolderPlus size={16} aria-hidden />
          New project
        </button>
      </header>
      {visible.length === 0 ? (
        <div className="empty-state">
          <h3>{EMPTY_STATES.noProjects.title}</h3>
          <p>{EMPTY_STATES.noProjects.body}</p>
          <div className="button-row empty-state-actions">
            <button type="button" className="btn btn-primary" onClick={onCreateProject}>
              {EMPTY_STATES.noProjects.primaryCta}
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => openHelpAndLearning(navigateToSettings)}
            >
              {EMPTY_STATES.helpLink.label}
            </button>
          </div>
        </div>
      ) : (
        <ul className="projects-grid">
          {visible.map((project) => (
            <li key={project.id}>
              <button
                type="button"
                className="project-grid-card"
                onClick={() => onOpenProject(project.id)}
              >
                <span className={projectIconClass(project.iconKey)} aria-hidden>
                  <Folder size={18} />
                </span>
                <strong>{project.name}</strong>
                {project.description ? (
                  <span className="muted">{project.description}</span>
                ) : null}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
