import { useMemo, useState } from "react";
import {
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Folder,
  FolderPlus,
  Image,
  MessageSquare,
  MessageSquarePlus,
  RefreshCw,
  Settings,
  Wrench,
} from "lucide-react";
import { CoresideLogo } from "@/components/branding/CoresideLogo";
import { ChatContextMenu } from "@/components/context-menu/ChatContextMenu";
import { ProjectContextMenu } from "@/components/context-menu/ProjectContextMenu";
import { useContextMenuTrigger } from "@/components/context-menu/use-context-menu";
import { ProjectList } from "@/components/projects/ProjectList";
import { EMPTY_STATES } from "@/lib/empty-states";
import { isCurrentChat } from "@/lib/navigation";
import type { Project } from "@/types/project";
import type { Conversation } from "@/types/messages";
import { useShallow } from "zustand/react/shallow";
import { useAppStore } from "@/stores/app-store";

const ICON_EXPANDED = 16;
const ICON_COLLAPSED = 22;
const LOGO_SIZE = 28;

export function Sidebar() {
  // Shallow-compared selector: the sidebar stays mounted during chat
  // streaming, so a full-store subscription would re-render it per token.
  const {
    sidebarCollapsed,
    toggleSidebar,
    view,
    conversations,
    activeConversationId,
    navigateToChat,
    createConversation,
    tools,
    activeToolId,
    selectTool,
    navigateToSettings,
    navigateToAutomations,
    navigateToMedia,
    navigateToProjects,
    navigateToProject,
    projects,
    projectsExpanded,
    setProjectsExpanded,
    activeProjectId,
    setCreateProjectDialogOpen,
    setEditProjectDialogOpen,
    setAddChatsDialogOpen,
    setDeleteProjectDialogOpen,
    setRenameConversationDialogOpen,
    createChatInProject,
    assignChats,
    removeChatFromProject,
    duplicateConversation,
    deleteConversation,
    archiveProject,
    restoreProject,
    resolvedTheme,
  } = useAppStore(
    useShallow((s) => ({
      sidebarCollapsed: s.sidebarCollapsed,
      toggleSidebar: s.toggleSidebar,
      view: s.view,
      conversations: s.conversations,
      activeConversationId: s.activeConversationId,
      navigateToChat: s.navigateToChat,
      createConversation: s.createConversation,
      tools: s.tools,
      activeToolId: s.activeToolId,
      selectTool: s.selectTool,
      navigateToSettings: s.navigateToSettings,
      navigateToAutomations: s.navigateToAutomations,
      navigateToMedia: s.navigateToMedia,
      navigateToProjects: s.navigateToProjects,
      navigateToProject: s.navigateToProject,
      projects: s.projects,
      projectsExpanded: s.projectsExpanded,
      setProjectsExpanded: s.setProjectsExpanded,
      activeProjectId: s.activeProjectId,
      setCreateProjectDialogOpen: s.setCreateProjectDialogOpen,
      setEditProjectDialogOpen: s.setEditProjectDialogOpen,
      setAddChatsDialogOpen: s.setAddChatsDialogOpen,
      setDeleteProjectDialogOpen: s.setDeleteProjectDialogOpen,
      setRenameConversationDialogOpen: s.setRenameConversationDialogOpen,
      createChatInProject: s.createChatInProject,
      assignChats: s.assignChats,
      removeChatFromProject: s.removeChatFromProject,
      duplicateConversation: s.duplicateConversation,
      deleteConversation: s.deleteConversation,
      archiveProject: s.archiveProject,
      restoreProject: s.restoreProject,
      resolvedTheme: s.resolvedTheme,
    })),
  );

  const iconSize = sidebarCollapsed ? ICON_COLLAPSED : ICON_EXPANDED;
  const chatMenu = useContextMenuTrigger();
  const projectMenu = useContextMenuTrigger();
  const [menuChat, setMenuChat] = useState<Conversation | null>(null);
  const [menuProject, setMenuProject] = useState<Project | null>(null);

  const projectNameById = useMemo(
    () => new Map(projects.map((p) => [p.id, p.name])),
    [projects],
  );

  const sidebarChats = useMemo(
    () =>
      [...conversations]
        .filter((c) => !c.archived)
        .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)),
    [conversations],
  );

  const openChatMenu = (chat: Conversation, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    setMenuChat(chat);
    chatMenu.openFromEvent(event);
  };

  const openProjectMenu = (project: Project, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    setMenuProject(project);
    projectMenu.openFromEvent(event);
  };

  const isChatActive = (id: string) =>
    isCurrentChat(view, activeConversationId, id);

  return (
    <aside
      className={`sidebar${sidebarCollapsed ? " collapsed" : ""}`}
      aria-label="Coreside sidebar"
    >
      <div className="sidebar-brand">
        {sidebarCollapsed ? (
          <button
            type="button"
            className="sidebar-brand-toggle"
            onClick={() => void toggleSidebar()}
            aria-label="Expand sidebar"
            aria-expanded={false}
          >
            <CoresideLogo
              appearance={resolvedTheme}
              size={LOGO_SIZE}
              className="wordmark-logo brand-logo"
            />
            <ChevronRight
              size={ICON_COLLAPSED}
              className="brand-expand-icon"
              aria-hidden
            />
          </button>
        ) : (
          <>
            <button
              type="button"
              className="wordmark"
              onClick={() => {
                if (activeConversationId) void navigateToChat(activeConversationId);
                else useAppStore.setState({ view: { kind: "chat", conversationId: null } });
              }}
              aria-label="Coreside home"
            >
              <CoresideLogo
                appearance={resolvedTheme}
                size={LOGO_SIZE}
                className="wordmark-logo"
              />
              <span className="wordmark-text">Coreside</span>
            </button>
            <button
              type="button"
              className="icon-btn"
              onClick={() => void toggleSidebar()}
              aria-label="Collapse sidebar"
              aria-expanded={true}
            >
              <ChevronLeft size={18} />
            </button>
          </>
        )}
      </div>

      <button
        type="button"
        className="btn btn-primary btn-block"
        onClick={() => void createConversation()}
        aria-label="New chat"
      >
        <MessageSquarePlus size={iconSize} aria-hidden />
        <span className="nav-label">New chat</span>
      </button>

      <section className="sidebar-section" aria-label="Projects" data-coreside-tour="sidebar-projects">
        <div className="sidebar-section-header">
          <button
            type="button"
            className="sidebar-section-toggle"
            onClick={() => setProjectsExpanded(!projectsExpanded)}
            aria-expanded={projectsExpanded}
          >
            <Folder size={iconSize} aria-hidden />
            <span className="nav-label">Projects</span>
            {!sidebarCollapsed ? (
              <ChevronDown
                size={14}
                className={`section-chevron${projectsExpanded ? " open" : ""}`}
                aria-hidden
              />
            ) : null}
          </button>
          {!sidebarCollapsed ? (
            <button
              type="button"
              className="sidebar-inline-btn"
              aria-label="New project"
              onClick={() => setCreateProjectDialogOpen(true)}
            >
              <FolderPlus size={14} />
            </button>
          ) : null}
        </div>
        {projectsExpanded ? (
          <div className="sidebar-nav">
            <ProjectList
              projects={projects}
              conversations={conversations}
              activeProjectId={activeProjectId}
              collapsed={sidebarCollapsed}
              onOpenProject={(id) => void navigateToProject(id)}
              onViewAllProject={(id) => void navigateToProject(id)}
              onChatOpen={(id) => void navigateToChat(id)}
              onProjectContextMenu={openProjectMenu}
              onProjectOverflow={openProjectMenu}
            />
            {!sidebarCollapsed ? (
              <button
                type="button"
                className="sidebar-nav-btn muted-link"
                onClick={() => navigateToProjects()}
              >
                View all projects
              </button>
            ) : null}
          </div>
        ) : null}
      </section>

      <section className="sidebar-section grow" aria-label="Recent chats">
        <div className="sidebar-section-label">Chats</div>
        <div className="sidebar-nav">
          {sidebarChats.length === 0 ? (
            <p className="muted sidebar-empty-hint">
              <span className="nav-label">{EMPTY_STATES.sidebarNoChats.hint}</span>
            </p>
          ) : (
            sidebarChats.map((conversation) => (
              <div key={conversation.id} className="sidebar-nav-row">
                <button
                  type="button"
                  className={`sidebar-nav-btn${isChatActive(conversation.id) ? " active" : ""}`}
                  onClick={() => void navigateToChat(conversation.id)}
                  onContextMenu={(event) => openChatMenu(conversation, event)}
                  aria-label={conversation.title}
                  title={conversation.title}
                >
                  {sidebarCollapsed ? (
                    <MessageSquare size={iconSize} aria-hidden />
                  ) : null}
                  <span className="nav-label">{conversation.title}</span>
                  {conversation.projectId && !sidebarCollapsed ? (
                    <span className="nav-badge">
                      {projectNameById.get(conversation.projectId) ?? "Project"}
                    </span>
                  ) : null}
                </button>
                {!sidebarCollapsed ? (
                  <button
                    type="button"
                    className="sidebar-overflow-btn"
                    aria-label={`More actions for ${conversation.title}`}
                    onClick={(event) => openChatMenu(conversation, event)}
                  >
                    <span aria-hidden>⋯</span>
                  </button>
                ) : null}
              </div>
            ))
          )}
        </div>
      </section>

      <section className="sidebar-section grow" aria-label="Personal apps">
        <div className="sidebar-section-label">Apps</div>
        <div className="sidebar-nav">
          {tools.length === 0 ? (
            <p className="muted sidebar-empty-hint">
              <span className="nav-label">{EMPTY_STATES.sidebarNoApps.hint}</span>
            </p>
          ) : (
            tools.map((tool) => (
              <button
                key={tool.id}
                type="button"
                className={`sidebar-nav-btn${
                  view.kind === "chat" && activeToolId === tool.id ? " active" : ""
                }`}
                onClick={() => void selectTool(tool.id)}
                aria-label={tool.name}
                title={tool.name}
              >
                <Wrench size={iconSize} aria-hidden />
                <span className="nav-label">{tool.name}</span>
                {tool.version != null ? (
                  <span className="nav-meta">v{tool.version}</span>
                ) : null}
              </button>
            ))
          )}
        </div>
      </section>

      <div className="sidebar-section">
        <button
          type="button"
          className={`sidebar-settings-btn${view.kind === "media" ? " active" : ""}`}
          onClick={() => navigateToMedia()}
          aria-label="Media library"
          title="Media library"
        >
          <Image size={iconSize} aria-hidden />
          <span className="nav-label">Media</span>
        </button>
        <button
          type="button"
          className={`sidebar-settings-btn${view.kind === "automations" ? " active" : ""}`}
          onClick={() => navigateToAutomations()}
          aria-label="Automations"
          title="Automations"
        >
          <RefreshCw size={iconSize} aria-hidden />
          <span className="nav-label">Automations</span>
        </button>
        <button
          type="button"
          className={`sidebar-settings-btn${view.kind === "settings" ? " active" : ""}`}
          onClick={() => navigateToSettings()}
          aria-label="Settings"
          title="Settings"
        >
          <Settings size={iconSize} aria-hidden />
          <span className="nav-label">Settings</span>
        </button>
      </div>

      {menuChat ? (
        <ChatContextMenu
          open={chatMenu.open}
          x={chatMenu.x}
          y={chatMenu.y}
          conversationId={menuChat.id}
          conversationTitle={menuChat.title}
          projectId={menuChat.projectId}
          projects={projects}
          isCurrent={isChatActive(menuChat.id)}
          onClose={chatMenu.close}
          onOpen={() => void navigateToChat(menuChat.id)}
          onRename={() => setRenameConversationDialogOpen(menuChat.id)}
          onAddToProject={(projectId) =>
            void assignChats(projectId, [menuChat.id])
          }
          onMoveToProject={(projectId) =>
            void assignChats(projectId, [menuChat.id])
          }
          onRemoveFromProject={() =>
            void removeChatFromProject(menuChat.id)
          }
          onOpenProject={() => {
            if (menuChat.projectId) void navigateToProject(menuChat.projectId);
          }}
          onDuplicate={() => void duplicateConversation(menuChat.id)}
          onExport={() => {
            window.alert("Chat export is not available yet.");
          }}
          onDelete={() => {
            if (window.confirm(`Delete "${menuChat.title}"?`)) {
              void deleteConversation(menuChat.id);
            }
          }}
        />
      ) : null}

      {menuProject ? (
        <ProjectContextMenu
          open={projectMenu.open}
          x={projectMenu.x}
          y={projectMenu.y}
          project={menuProject}
          onClose={projectMenu.close}
          onOpen={() => void navigateToProject(menuProject.id)}
          onNewChat={() => void createChatInProject(menuProject.id)}
          onAddChats={() => setAddChatsDialogOpen(menuProject.id)}
          onRename={() => setEditProjectDialogOpen(menuProject.id)}
          onEditInstructions={() => setEditProjectDialogOpen(menuProject.id)}
          onManageContext={() => void navigateToProject(menuProject.id)}
          onExport={() => {
            const path = window.prompt(
              "Save project export to path:",
              `${menuProject.name}.json`,
            );
            if (path) {
              void import("@/lib/tauri").then(({ api }) =>
                api.exportProject(menuProject.id, path),
              );
            }
          }}
          onArchiveToggle={() =>
            void (menuProject.archived
              ? restoreProject(menuProject.id)
              : archiveProject(menuProject.id))
          }
          onDelete={() => setDeleteProjectDialogOpen(menuProject.id)}
        />
      ) : null}
    </aside>
  );
}
