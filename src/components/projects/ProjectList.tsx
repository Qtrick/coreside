import { Folder } from "lucide-react";
import type { Project } from "@/types/project";
import type { Conversation } from "@/types/messages";
import { projectIconClass } from "./project-icon";

type ProjectListItemProps = {
  project: Project;
  active: boolean;
  recentChats?: Conversation[];
  collapsed?: boolean;
  onOpen: () => void;
  onViewAll: () => void;
  onChatOpen: (conversationId: string) => void;
  onContextMenu: (event: React.MouseEvent) => void;
  onOverflowClick: (event: React.MouseEvent) => void;
};

export function ProjectListItem({
  project,
  active,
  recentChats = [],
  collapsed,
  onOpen,
  onViewAll,
  onChatOpen,
  onContextMenu,
  onOverflowClick,
}: ProjectListItemProps) {
  return (
    <div className="project-list-item">
      <div className="sidebar-nav-row">
        <button
          type="button"
          className={`sidebar-nav-btn${active ? " active" : ""}`}
          onClick={onOpen}
          onContextMenu={onContextMenu}
          aria-label={project.name}
          title={project.name}
        >
          <span className={projectIconClass(project.iconKey)} aria-hidden>
            <Folder size={16} />
          </span>
          {!collapsed ? (
            <span className="nav-label">{project.name}</span>
          ) : null}
        </button>
        {!collapsed ? (
          <button
            type="button"
            className="sidebar-overflow-btn"
            aria-label={`More actions for ${project.name}`}
            onClick={onOverflowClick}
          >
            <span aria-hidden>⋯</span>
          </button>
        ) : null}
      </div>
      {!collapsed && recentChats.length > 0 ? (
        <div className="project-nested-chats">
          {recentChats.slice(0, 3).map((chat) => (
            <button
              key={chat.id}
              type="button"
              className="sidebar-nav-btn nested"
              onClick={() => onChatOpen(chat.id)}
            >
              <span className="nav-label">{chat.title}</span>
            </button>
          ))}
          {recentChats.length > 0 ? (
            <button
              type="button"
              className="sidebar-nav-btn nested muted-link"
              onClick={onViewAll}
            >
              View all
            </button>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

type ProjectListProps = {
  projects: Project[];
  conversations: Conversation[];
  activeProjectId: string | null;
  collapsed?: boolean;
  onOpenProject: (projectId: string) => void;
  onViewAllProject: (projectId: string) => void;
  onChatOpen: (conversationId: string) => void;
  onProjectContextMenu: (
    project: Project,
    event: React.MouseEvent,
  ) => void;
  onProjectOverflow: (
    project: Project,
    event: React.MouseEvent,
  ) => void;
};

export function ProjectList({
  projects,
  conversations,
  activeProjectId,
  collapsed,
  onOpenProject,
  onViewAllProject,
  onChatOpen,
  onProjectContextMenu,
  onProjectOverflow,
}: ProjectListProps) {
  const visible = projects.filter((p) => !p.archived);

  if (visible.length === 0) {
    return (
      <p className="muted sidebar-empty-hint">
        <span className="nav-label">No projects yet</span>
      </p>
    );
  }

  return (
    <>
      {visible.map((project) => {
        const recentChats = conversations
          .filter((c) => c.projectId === project.id)
          .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
        return (
          <ProjectListItem
            key={project.id}
            project={project}
            active={activeProjectId === project.id}
            recentChats={recentChats}
            collapsed={collapsed}
            onOpen={() => onOpenProject(project.id)}
            onViewAll={() => onViewAllProject(project.id)}
            onChatOpen={onChatOpen}
            onContextMenu={(event) => onProjectContextMenu(project, event)}
            onOverflowClick={(event) => onProjectOverflow(project, event)}
          />
        );
      })}
    </>
  );
}
