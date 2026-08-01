import { Folder } from "lucide-react";
import type { Project } from "@/types/project";
import { projectIconClass } from "@/components/projects/project-icon";

type ProjectIndicatorProps = {
  project: Project;
  onClick: () => void;
};

export function ProjectIndicator({ project, onClick }: ProjectIndicatorProps) {
  return (
    <button
      type="button"
      className="project-indicator"
      onClick={onClick}
      aria-label={`Open project ${project.name}`}
      title={project.name}
    >
      <span className={projectIconClass(project.iconKey)} aria-hidden>
        <Folder size={14} />
      </span>
      <span>{project.name}</span>
    </button>
  );
}
