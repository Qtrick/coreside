import { useState, useMemo, useRef, useEffect, type ChangeEvent, type KeyboardEvent } from "react";
import {
  ChevronDown,
  ChevronLeft,
  Folder,
  FolderPlus,
  LayoutGrid,
  MessageSquare,
  Plus,
  Search,
  Wrench,
  X,
} from "lucide-react";
import { CoresideLogo } from "@/components/branding/CoresideLogo";
import { ChatContextMenu } from "@/components/context-menu/ChatContextMenu";
import { ProjectContextMenu } from "@/components/context-menu/ProjectContextMenu";
import { useContextMenuTrigger } from "@/components/context-menu/use-context-menu";
import { ProjectList } from "@/components/projects/ProjectList";
import { EMPTY_STATES } from "@/lib/empty-states";
import { isCurrentChat } from "@/lib/navigation";
import { formatRelativeTime } from "@/lib/format-relative-time";
import { performUnifiedSearch, type SearchResultItem } from "@/lib/search/unified-search";
import type { Project } from "@/types/project";
import type { Conversation } from "@/types/messages";
import { useShallow } from "zustand/react/shallow";
import { useAppStore } from "@/stores/app-store";
import { api } from "@/lib/tauri";

export type ContextualMode = "chats_projects" | "apps";

interface ContextualSidebarProps {
  mode: ContextualMode;
  onModeChange: (mode: ContextualMode) => void;
  onCollapse: () => void;
  collapsed?: boolean;
}

export function ContextualSidebar({
  mode,
  onModeChange,
  onCollapse,
  collapsed = false,
}: ContextualSidebarProps) {
  const {
    view,
    conversations,
    activeConversationId,
    navigateToChat,
    createConversation,
    tools,
    activeToolId,
    selectTool,
    projects,
    projectsExpanded,
    setProjectsExpanded,
    activeProjectId,
    setCreateProjectDialogOpen,
    setRenameConversationDialogOpen,
    setDeleteConversationDialogOpen,
    assignChats,
    removeChatFromProject,
    duplicateConversation,
    resolvedTheme,
    navigateToProject,
    navigateToProjects,
  } = useAppStore(
    useShallow((s) => ({
      view: s.view,
      conversations: s.conversations,
      activeConversationId: s.activeConversationId,
      navigateToChat: s.navigateToChat,
      createConversation: s.createConversation,
      tools: s.tools,
      activeToolId: s.activeToolId,
      selectTool: s.selectTool,
      projects: s.projects,
      projectsExpanded: s.projectsExpanded,
      setProjectsExpanded: s.setProjectsExpanded,
      activeProjectId: s.activeProjectId,
      setCreateProjectDialogOpen: s.setCreateProjectDialogOpen,
      setRenameConversationDialogOpen: s.setRenameConversationDialogOpen,
      setDeleteConversationDialogOpen: s.setDeleteConversationDialogOpen,
      assignChats: s.assignChats,
      removeChatFromProject: s.removeChatFromProject,
      duplicateConversation: s.duplicateConversation,
      resolvedTheme: s.resolvedTheme,
      navigateToProject: s.navigateToProject,
      navigateToProjects: s.navigateToProjects,
    })),
  );

  const [searchQuery, setSearchQuery] = useState("");
  const [searchFocused, setSearchFocused] = useState(false);
  const [selectedSearchIndex, setSelectedSearchIndex] = useState(0);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const searchBoxRef = useRef<HTMLDivElement>(null);

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

  // Unified search filtering
  const searchResults = useMemo(() => {
    return performUnifiedSearch({
      query: searchQuery,
      conversations,
      projects,
      tools,
    });
  }, [searchQuery, conversations, projects, tools]);

  const flatSearchResults = useMemo(() => {
    return [
      ...searchResults.chats,
      ...searchResults.projects,
      ...searchResults.apps,
    ];
  }, [searchResults]);

  useEffect(() => {
    setSelectedSearchIndex(0);
  }, [searchQuery]);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (
        searchBoxRef.current &&
        !searchBoxRef.current.contains(event.target as Node)
      ) {
        setSearchFocused(false);
      }
    }
    if (searchFocused) {
      document.addEventListener("mousedown", handleClickOutside);
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [searchFocused]);

  // Global ⌘K shortcut focuses search input
  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

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

  const handleSearchResultClick = (item: SearchResultItem) => {
    if (item.type === "chat") {
      void navigateToChat(item.id);
    } else if (item.type === "project") {
      void navigateToProject(item.id);
    } else if (item.type === "app") {
      void selectTool(item.id);
    }
    setSearchQuery("");
    setSearchFocused(false);
  };

  const handleSearchKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      setSearchQuery("");
      setSearchFocused(false);
      searchInputRef.current?.blur();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedSearchIndex((prev) =>
        flatSearchResults.length > 0
          ? (prev + 1) % flatSearchResults.length
          : 0,
      );
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedSearchIndex((prev) =>
        flatSearchResults.length > 0
          ? (prev - 1 + flatSearchResults.length) % flatSearchResults.length
          : 0,
      );
    } else if (e.key === "Enter" && flatSearchResults.length > 0) {
      e.preventDefault();
      const selected = flatSearchResults[selectedSearchIndex];
      if (selected) {
        handleSearchResultClick(selected);
      }
    }
  };

  if (collapsed) return null;

  return (
    <aside
      className="contextual-sidebar"
      aria-label="Contextual workspace sidebar"
      data-testid="contextual-sidebar"
    >
      {/* Top Header */}
      <div className="contextual-header">
        <div className="contextual-brand">
          <CoresideLogo
            appearance={resolvedTheme}
            size={22}
            className="brand-logo"
          />
          <span className="contextual-title">Coreside</span>
        </div>
        <button
          type="button"
          className="contextual-collapse-btn"
          onClick={onCollapse}
          aria-label="Collapse contextual sidebar"
          title="Collapse sidebar"
        >
          <ChevronLeft size={16} />
        </button>
      </div>

      {/* Search Input Bar */}
      <div className="contextual-search-box" ref={searchBoxRef}>
        <Search size={14} className="search-icon" aria-hidden />
        <input
          ref={searchInputRef}
          type="search"
          className="contextual-search-input"
          placeholder="Search chats, projects, apps..."
          value={searchQuery}
          onChange={(e: ChangeEvent<HTMLInputElement>) =>
            setSearchQuery(e.target.value)
          }
          onFocus={() => setSearchFocused(true)}
          onKeyDown={handleSearchKeyDown}
          aria-label="Search chats, projects, and apps"
        />
        {searchQuery ? (
          <button
            type="button"
            className="search-clear-btn"
            onClick={() => setSearchQuery("")}
            aria-label="Clear search"
          >
            <X size={12} />
          </button>
        ) : (
          <kbd className="search-kbd" aria-hidden>⌘K</kbd>
        )}

        {/* Live Search Results Dropdown Overlay */}
        {searchFocused && searchQuery.trim() ? (
          <div
            className="contextual-search-results"
            role="listbox"
            aria-label="Search results"
          >
            {searchResults.totalCount === 0 ? (
              <div className="search-no-results">
                No matching chats, projects, or apps
              </div>
            ) : (
              <>
                {searchResults.chats.length > 0 ? (
                  <div className="search-group">
                    <div className="search-group-header">Chats</div>
                    {searchResults.chats.map((item) => {
                      const idx = flatSearchResults.indexOf(item);
                      return (
                        <button
                          key={item.id}
                          type="button"
                          className={`search-result-row${idx === selectedSearchIndex ? " selected" : ""}`}
                          onClick={() => handleSearchResultClick(item)}
                          role="option"
                          aria-selected={idx === selectedSearchIndex}
                        >
                          <MessageSquare size={13} aria-hidden />
                          <span className="search-result-title">{item.title}</span>
                          {item.updatedAt ? (
                            <span className="search-result-meta">
                              {formatRelativeTime(item.updatedAt)}
                            </span>
                          ) : null}
                        </button>
                      );
                    })}
                  </div>
                ) : null}

                {searchResults.projects.length > 0 ? (
                  <div className="search-group">
                    <div className="search-group-header">Projects</div>
                    {searchResults.projects.map((item) => {
                      const idx = flatSearchResults.indexOf(item);
                      return (
                        <button
                          key={item.id}
                          type="button"
                          className={`search-result-row${idx === selectedSearchIndex ? " selected" : ""}`}
                          onClick={() => handleSearchResultClick(item)}
                          role="option"
                          aria-selected={idx === selectedSearchIndex}
                        >
                          <Folder size={13} aria-hidden />
                          <span className="search-result-title">{item.title}</span>
                        </button>
                      );
                    })}
                  </div>
                ) : null}

                {searchResults.apps.length > 0 ? (
                  <div className="search-group">
                    <div className="search-group-header">Apps</div>
                    {searchResults.apps.map((item) => {
                      const idx = flatSearchResults.indexOf(item);
                      return (
                        <button
                          key={item.id}
                          type="button"
                          className={`search-result-row${idx === selectedSearchIndex ? " selected" : ""}`}
                          onClick={() => handleSearchResultClick(item)}
                          role="option"
                          aria-selected={idx === selectedSearchIndex}
                        >
                          <Wrench size={13} aria-hidden />
                          <span className="search-result-title">{item.title}</span>
                          {item.subtitle ? (
                            <span className="search-result-meta">{item.subtitle}</span>
                          ) : null}
                        </button>
                      );
                    })}
                  </div>
                ) : null}
              </>
            )}
          </div>
        ) : null}
      </div>

      {/* Prominent + New Chat Button */}
      <button
        type="button"
        className="contextual-new-chat-btn"
        onClick={() => void createConversation()}
        aria-label="New chat"
        data-coreside-tour="sidebar-new-chat"
      >
        <Plus size={16} aria-hidden />
        <span>New chat</span>
      </button>

      {/* Segmented Switcher: Exactly Chats & Projects | Apps */}
      <div
        className="contextual-segmented-switcher"
        role="tablist"
        aria-label="Sidebar view modes"
      >
        <button
          type="button"
          role="tab"
          aria-selected={mode === "chats_projects"}
          className={`contextual-tab${mode === "chats_projects" ? " active" : ""}`}
          onClick={() => onModeChange("chats_projects")}
        >
          <Folder size={14} aria-hidden />
          <span>Chats & Projects</span>
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={mode === "apps"}
          className={`contextual-tab${mode === "apps" ? " active" : ""}`}
          onClick={() => onModeChange("apps")}
        >
          <LayoutGrid size={14} aria-hidden />
          <span>Apps</span>
        </button>
      </div>

      {/* Content for Chats & Projects */}
      {mode === "chats_projects" ? (
        <div className="contextual-content">
          {/* Projects Section */}
          <section
            className="sidebar-section projects-section"
            aria-label="Projects"
            data-coreside-tour="sidebar-projects"
          >
            <div className="sidebar-section-header">
              <button
                type="button"
                className="sidebar-section-toggle"
                onClick={() => setProjectsExpanded(!projectsExpanded)}
                aria-expanded={projectsExpanded}
                aria-label="Projects"
              >
                <Folder size={15} aria-hidden />
                <span className="nav-label">PROJECTS</span>
                <ChevronDown
                  size={13}
                  className={`section-chevron${projectsExpanded ? " open" : ""}`}
                  aria-hidden
                />
              </button>
              <button
                type="button"
                className="sidebar-inline-btn"
                aria-label="New project"
                title="Create project"
                onClick={() => setCreateProjectDialogOpen(true)}
              >
                <FolderPlus size={14} />
              </button>
            </div>
            {projectsExpanded ? (
              <div className="sidebar-nav">
                <ProjectList
                  projects={projects}
                  conversations={conversations}
                  activeProjectId={activeProjectId}
                  collapsed={false}
                  onOpenProject={(id) => void navigateToProject(id)}
                  onViewAllProject={(id) => void navigateToProject(id)}
                  onChatOpen={(id) => void navigateToChat(id)}
                  onProjectContextMenu={openProjectMenu}
                  onProjectOverflow={openProjectMenu}
                />
                <button
                  type="button"
                  className="sidebar-nav-btn muted-link"
                  onClick={() => navigateToProjects()}
                >
                  View all projects
                </button>
              </div>
            ) : null}
          </section>

          {/* Chats Section */}
          <section className="sidebar-section grow chats-section" aria-label="Recent chats">
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
                      <MessageSquare size={14} aria-hidden />
                      <span className="nav-label">{conversation.title}</span>
                      {conversation.projectId ? (
                        <span className="nav-badge">
                          {projectNameById.get(conversation.projectId) ?? "Project"}
                        </span>
                      ) : null}
                      <span className="nav-time">
                        {formatRelativeTime(conversation.updatedAt)}
                      </span>
                    </button>
                    <button
                      type="button"
                      className="sidebar-overflow-btn"
                      aria-label={`More actions for ${conversation.title}`}
                      onClick={(event) => openChatMenu(conversation, event)}
                    >
                      <span aria-hidden>⋯</span>
                    </button>
                  </div>
                ))
              )}
            </div>
          </section>
        </div>
      ) : null}

      {/* Content for Apps */}
      {mode === "apps" ? (
        <div className="contextual-content">
          <section
            className="sidebar-section grow apps-section"
            aria-label="Personal apps"
            data-coreside-tour="sidebar-apps"
          >
            <div className="sidebar-section-label">Personal Apps</div>
            <div className="sidebar-nav">
              {tools.length === 0 ? (
                <div className="sidebar-empty-state">
                  <Wrench size={24} className="empty-icon" aria-hidden />
                  <p className="muted sidebar-empty-hint">
                    {EMPTY_STATES.sidebarNoApps.hint}
                  </p>
                </div>
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
                    <Wrench size={14} aria-hidden />
                    <span className="nav-label">{tool.name}</span>
                    {tool.version != null ? (
                      <span className="nav-meta">v{tool.version}</span>
                    ) : null}
                  </button>
                ))
              )}
            </div>
          </section>
        </div>
      ) : null}

      {/* Context Menus */}
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
          onExport={async () => {
            try {
              const messages = await api.getMessages(menuChat.id);
              const exportData = {
                conversationId: menuChat.id,
                title: menuChat.title,
                exportedAt: new Date().toISOString(),
                messages,
              };
              const blob = new Blob([JSON.stringify(exportData, null, 2)], {
                type: "application/json",
              });
              const url = URL.createObjectURL(blob);
              const a = document.createElement("a");
              a.href = url;
              const safeTitle =
                menuChat.title.replace(/[^a-zA-Z0-9_-]/g, "_") || "chat";
              a.download = `${safeTitle}-export.json`;
              document.body.appendChild(a);
              a.click();
              document.body.removeChild(a);
              URL.revokeObjectURL(url);
            } catch (err) {
              console.error("Export failed", err);
            }
          }}
          onDelete={() => setDeleteConversationDialogOpen(menuChat.id)}
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
          onNewChat={() => void useAppStore.getState().createChatInProject(menuProject.id)}
          onAddChats={() => useAppStore.getState().setAddChatsDialogOpen(menuProject.id)}
          onRename={() => useAppStore.getState().setEditProjectDialogOpen(menuProject.id)}
          onEditInstructions={() => useAppStore.getState().setEditProjectDialogOpen(menuProject.id)}
          onManageContext={() =>
            useAppStore.getState().setManageContextDialogOpen(menuProject.id)
          }
          onExport={() => {
            const path = window.prompt(
              "Export file name (saved under Coreside exports):",
              `${menuProject.name}.coreside-project.json`,
            );
            if (path) {
              void import("@/lib/tauri").then(({ api }) =>
                api.exportProject(menuProject.id, path),
              );
            }
          }}
          onArchiveToggle={() =>
            void (menuProject.archived
              ? useAppStore.getState().restoreProject(menuProject.id)
              : useAppStore.getState().archiveProject(menuProject.id))
          }
          onDelete={() =>
            useAppStore.getState().setDeleteProjectDialogOpen(menuProject.id)
          }
        />
      ) : null}
    </aside>
  );
}
