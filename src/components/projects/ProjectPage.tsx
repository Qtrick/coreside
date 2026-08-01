import { Folder, Image, MessageSquarePlus, Wallpaper } from "lucide-react";
import type { Project } from "@/types/project";
import type { Conversation } from "@/types/messages";
import { projectIconClass } from "./project-icon";
import { useAppStore } from "@/stores/app-store";

type ProjectPageProps = {
  project: Project;
  conversations: Conversation[];
  onBack: () => void;
  onNewChat: () => void;
  onOpenChat: (conversationId: string) => void;
  onEditProject: () => void;
  onManageContext: () => void;
  onAddChats: () => void;
};

export function ProjectPage({
  project,
  conversations,
  onBack,
  onNewChat,
  onOpenChat,
  onEditProject,
  onManageContext,
  onAddChats,
}: ProjectPageProps) {
  const navigateToMedia = useAppStore((s) => s.navigateToMedia);
  const sorted = [...conversations].sort((a, b) =>
    b.updatedAt.localeCompare(a.updatedAt),
  );

  return (
    <section className="project-page" aria-label={project.name}>
      <header className="panel-header project-page-header">
        <div>
          <button type="button" className="text-btn" onClick={onBack}>
            ← Projects
          </button>
          <div className="project-page-title">
            <span className={projectIconClass(project.iconKey)} aria-hidden>
              <Folder size={20} />
            </span>
            <div>
              <h1>{project.name}</h1>
              {project.description ? (
                <p className="panel-subtitle">{project.description}</p>
              ) : null}
            </div>
          </div>
        </div>
        <div className="project-page-actions">
          <button type="button" className="btn btn-primary" onClick={onNewChat}>
            <MessageSquarePlus size={16} aria-hidden />
            New chat
          </button>
          <button type="button" className="btn btn-secondary" onClick={onAddChats}>
            Add chats
          </button>
          <button type="button" className="btn btn-secondary" onClick={onEditProject}>
            Edit
          </button>
        </div>
      </header>

      <div className="project-page-body">
        <section className="project-card">
          <h2>Instructions</h2>
          {project.instructions?.trim() ? (
            <p className="project-instructions">{project.instructions}</p>
          ) : (
            <p className="muted">
              No instructions yet.{" "}
              <button type="button" className="text-btn" onClick={onEditProject}>
                Add instructions
              </button>
            </p>
          )}
        </section>

        <section className="project-card">
          <div className="project-card-header">
            <h2>Recent chats</h2>
            <button type="button" className="text-btn" onClick={onManageContext}>
              Manage context
            </button>
          </div>
          {sorted.length === 0 ? (
            <p className="muted">No chats in this project yet.</p>
          ) : (
            <ul className="project-chat-list">
              {sorted.map((chat) => (
                <li key={chat.id}>
                  <button
                    type="button"
                    className="project-chat-row"
                    onClick={() => onOpenChat(chat.id)}
                  >
                    <span>{chat.title}</span>
                    <time dateTime={chat.updatedAt}>
                      {new Date(chat.updatedAt).toLocaleDateString()}
                    </time>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="project-card">
          <div className="project-card-header">
            <h2>Wallpaper & media</h2>
          </div>
          <p className="muted">
            {project.wallpaperJson
              ? "This project has a custom wallpaper for its chats."
              : "No project wallpaper yet. Import media and apply it here or from the media library."}
          </p>
          <div className="button-row">
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => navigateToMedia()}
            >
              <Image size={16} aria-hidden />
              Open media library
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => navigateToMedia()}
            >
              <Wallpaper size={16} aria-hidden />
              Set wallpaper
            </button>
          </div>
        </section>
      </div>
    </section>
  );
}
