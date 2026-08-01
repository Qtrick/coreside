import { Folder, FolderPlus } from "lucide-react";
import type { Project } from "@/types/project";
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
          <h3>No projects yet</h3>
          <p>Create a project to group related chats.</p>
          <button type="button" className="btn btn-primary" onClick={onCreateProject}>
            Create project
          </button>
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
