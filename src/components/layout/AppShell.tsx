import { Sidebar } from "@/components/sidebar/Sidebar";
import { ChatPanel } from "@/components/chat/ChatPanel";
import { ToolCanvas } from "@/components/tool-canvas/ToolCanvas";
import { SettingsPanel } from "@/components/settings/SettingsPanel";
import { AutomationsPanel } from "@/components/automations/AutomationsPanel";
import { ProviderSetupDialog } from "@/components/providers/ProviderSetupDialog";
import { ExportDialog } from "@/components/export/ExportDialog";
import { LiveWallpaper } from "@/components/wallpaper/LiveWallpaper";
import { MediaLibrary } from "@/components/media/MediaLibrary";
import { ProjectPage } from "@/components/projects/ProjectPage";
import { ProjectsListPage } from "@/components/projects/ProjectsListPage";
import { ChatToolSplitter } from "@/components/layout/ChatToolSplitter";
import { resolveActiveWallpaper } from "@/lib/wallpaper";
import { wallpaperDataAttribute } from "@/types/wallpaper";
import {
  classifyLayoutMode,
  clampSplitForWidth,
  clampSplitRatio,
  LAYOUT_SAFE,
} from "@/lib/layout-mode";
import {
  applyInterfaceTransparencyCssVars,
  clampInterfaceTransparency,
  computeInterfaceTransparencyTokens,
} from "@/lib/interface-transparency";
import { CreateProjectDialog } from "@/components/projects/CreateProjectDialog";
import { EditProjectDialog } from "@/components/projects/EditProjectDialog";
import { AddChatsToProjectDialog } from "@/components/projects/AddChatsToProjectDialog";
import { DeleteProjectDialog } from "@/components/projects/DeleteProjectDialog";
import { RenameConversationDialog } from "@/components/projects/RenameConversationDialog";
import { CommandPalette } from "@/components/CommandPalette";
import { PendingApprovalsHost } from "@/components/applications/PendingApprovalsHost";
import { WelcomeDialog } from "@/components/onboarding/WelcomeDialog";
import { TutorialOverlay } from "@/components/onboarding/TutorialOverlay";
import { ContextualEducationHost } from "@/components/onboarding/ContextualEducationHost";
import { useOnboardingStore } from "@/stores/onboarding-store";
import { useAppStore } from "@/stores/app-store";
import { useEffect, useRef, useState, type CSSProperties } from "react";

export function AppShell() {
  const [commandOpen, setCommandOpen] = useState(false);
  const [mainWidth, setMainWidth] = useState(0);
  const [liveSplit, setLiveSplit] = useState<number | null>(null);
  const mainRef = useRef<HTMLDivElement>(null);

  const sidebarCollapsed = useAppStore((s) => s.sidebarCollapsed);
  const view = useAppStore((s) => s.view);
  const activeToolId = useAppStore((s) => s.activeToolId);
  const activeTool = useAppStore((s) => s.activeTool);
  const exportDialog = useAppStore((s) => s.exportDialog);
  const closeExportDialog = useAppStore((s) => s.closeExportDialog);
  const wallpaper = useAppStore((s) => s.wallpaper);
  const globalWallpaperJson = useAppStore((s) => s.globalWallpaperJson);
  const interfaceTransparency = useAppStore((s) => s.interfaceTransparency);
  const chatToolSplitRatio = useAppStore((s) => s.chatToolSplitRatio);
  const setChatToolSplitRatio = useAppStore((s) => s.setChatToolSplitRatio);
  const layoutMode = useAppStore((s) => s.layoutMode);
  const setLayoutMode = useAppStore((s) => s.setLayoutMode);
  const projects = useAppStore((s) => s.projects);
  const activeProject = useAppStore((s) => s.activeProject);
  const projectConversations = useAppStore((s) => s.projectConversations);
  const conversations = useAppStore((s) => s.conversations);
  const activeConversationId = useAppStore((s) => s.activeConversationId);

  const resolvedWallpaper = resolveActiveWallpaper({
    view,
    globalWallpaper: wallpaper,
    globalWallpaperJson,
    conversations,
    projects,
    activeConversationId,
    activeProject,
  });
  const wallpaperAttr = wallpaperDataAttribute(resolvedWallpaper);
  const wallpaperActive = Boolean(wallpaperAttr);

  const createProjectDialogOpen = useAppStore((s) => s.createProjectDialogOpen);
  const setCreateProjectDialogOpen = useAppStore((s) => s.setCreateProjectDialogOpen);
  const createProject = useAppStore((s) => s.createProject);
  const projectActionBusy = useAppStore((s) => s.projectActionBusy);
  const projectActionError = useAppStore((s) => s.projectActionError);

  const editProjectDialogOpen = useAppStore((s) => s.editProjectDialogOpen);
  const editProjectId = useAppStore((s) => s.editProjectId);
  const setEditProjectDialogOpen = useAppStore((s) => s.setEditProjectDialogOpen);
  const updateProject = useAppStore((s) => s.updateProject);

  const addChatsDialogOpen = useAppStore((s) => s.addChatsDialogOpen);
  const addChatsProjectId = useAppStore((s) => s.addChatsProjectId);
  const setAddChatsDialogOpen = useAppStore((s) => s.setAddChatsDialogOpen);
  const assignChats = useAppStore((s) => s.assignChats);

  const deleteProjectDialogOpen = useAppStore((s) => s.deleteProjectDialogOpen);
  const deleteProjectId = useAppStore((s) => s.deleteProjectId);
  const setDeleteProjectDialogOpen = useAppStore((s) => s.setDeleteProjectDialogOpen);
  const deleteProject = useAppStore((s) => s.deleteProject);

  const renameConversationDialogOpen = useAppStore(
    (s) => s.renameConversationDialogOpen,
  );
  const renameConversationId = useAppStore((s) => s.renameConversationId);
  const setRenameConversationDialogOpen = useAppStore(
    (s) => s.setRenameConversationDialogOpen,
  );
  const renameConversation = useAppStore((s) => s.renameConversation);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setCommandOpen((open) => !open);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    void useOnboardingStore.getState().hydrate().then(() => {
      void useOnboardingStore.getState().openWelcomeIfEligible();
    });
  }, []);

  useEffect(() => {
    const tokens = computeInterfaceTransparencyTokens(
      clampInterfaceTransparency(interfaceTransparency),
      { wallpaperActive },
    );
    applyInterfaceTransparencyCssVars(tokens);
    document.body.dataset.wallpaperActive = wallpaperActive ? "true" : "false";
  }, [interfaceTransparency, wallpaperActive]);

  const navigateToProjects = useAppStore((s) => s.navigateToProjects);
  const navigateToProject = useAppStore((s) => s.navigateToProject);
  const createChatInProject = useAppStore((s) => s.createChatInProject);
  const navigateToChat = useAppStore((s) => s.navigateToChat);

  const overlayMode =
    view.kind === "settings" ||
    view.kind === "automations" ||
    view.kind === "projects" ||
    view.kind === "media" ||
    view.kind === "project";

  const showToolCanvas =
    view.kind === "chat" && activeToolId && !overlayMode;

  const splitRatio = clampSplitForWidth(
    liveSplit ?? chatToolSplitRatio,
    mainWidth || 1,
  );

  useEffect(() => {
    const el = mainRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    let frame = 0;
    const ro = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const width = entry.contentRect.width;
        setMainWidth(width);
        if (!showToolCanvas) {
          if (layoutMode !== "wide" && layoutMode !== "standard") {
            setLayoutMode(
              width >= LAYOUT_SAFE.standardMainThreshold ? "wide" : "standard",
            );
          }
          return;
        }
        const next = classifyLayoutMode({
          shellWidth: window.innerWidth,
          mainWidth: width,
          sidebarWidth: sidebarCollapsed ? 80 : 272,
          toolOpen: true,
          splitRatio: clampSplitRatio(chatToolSplitRatio),
        });
        if (next !== layoutMode) setLayoutMode(next);
      });
    });
    ro.observe(el);
    return () => {
      cancelAnimationFrame(frame);
      ro.disconnect();
    };
  }, [
    showToolCanvas,
    sidebarCollapsed,
    chatToolSplitRatio,
    layoutMode,
    setLayoutMode,
  ]);

  const classes = [
    "app-shell",
    sidebarCollapsed ? "sidebar-collapsed" : "",
    !showToolCanvas ? "no-tool" : "",
    overlayMode ? "settings-mode" : "",
    layoutMode === "compact" && showToolCanvas ? "layout-compact" : "",
  ]
    .filter(Boolean)
    .join(" ");

  const mainClasses = [
    "app-shell-main",
    showToolCanvas ? "has-tool" : "",
    layoutMode === "compact" && showToolCanvas ? "compact" : "",
  ]
    .filter(Boolean)
    .join(" ");

  const editProject =
    projects.find((p) => p.id === editProjectId) ?? activeProject;
  const addChatsProject =
    projects.find((p) => p.id === addChatsProjectId) ?? null;
  const deleteProjectTarget =
    projects.find((p) => p.id === deleteProjectId) ?? null;
  const renameTarget = conversations.find((c) => c.id === renameConversationId);

  const chatToolMain = (
    <div
      ref={mainRef}
      className={mainClasses}
      style={
        showToolCanvas && layoutMode !== "compact"
          ? ({
              "--chat-split-fr": `${splitRatio}fr`,
              "--tool-split-fr": `${1 - splitRatio}fr`,
            } as CSSProperties)
          : undefined
      }
    >
      <ChatPanel />
      {showToolCanvas && layoutMode !== "compact" ? (
        <ChatToolSplitter
          ratio={splitRatio}
          mainWidth={mainWidth}
          onChange={setLiveSplit}
          onCommit={(r) => {
            setLiveSplit(null);
            void setChatToolSplitRatio(r);
          }}
        />
      ) : null}
      {showToolCanvas ? <ToolCanvas /> : null}
    </div>
  );

  const mainPanel = (() => {
    switch (view.kind) {
      case "settings":
        return <SettingsPanel />;
      case "automations":
        return (
          <AutomationsPanel
            onBack={() => {
              const id = useAppStore.getState().activeConversationId;
              if (id) void navigateToChat(id);
              else useAppStore.setState({ view: { kind: "chat", conversationId: null } });
            }}
          />
        );
      case "projects":
        return (
          <ProjectsListPage
            projects={projects}
            onCreateProject={() => setCreateProjectDialogOpen(true)}
            onOpenProject={(id) => void navigateToProject(id)}
          />
        );
      case "media":
        return <MediaLibrary />;
      case "project":
        return activeProject ? (
          <ProjectPage
            project={activeProject}
            conversations={projectConversations}
            onBack={() => navigateToProjects()}
            onNewChat={() => void createChatInProject(activeProject.id)}
            onOpenChat={(id) => void navigateToChat(id)}
            onEditProject={() => setEditProjectDialogOpen(activeProject.id)}
            onManageContext={() => {
              /* Project context search arrives in a later phase — no browser alert. */
            }}
            onAddChats={() => setAddChatsDialogOpen(activeProject.id)}
          />
        ) : (
          <ProjectsListPage
            projects={projects}
            onCreateProject={() => setCreateProjectDialogOpen(true)}
            onOpenProject={(id) => void navigateToProject(id)}
          />
        );
      default:
        return chatToolMain;
    }
  })();

  return (
    <div
      className={classes}
      data-wallpaper={wallpaperAttr}
      data-layout-mode={layoutMode}
    >
      <LiveWallpaper wallpaper={resolvedWallpaper} />
      <Sidebar />
      {mainPanel}
      <ProviderSetupDialog />
      {exportDialog ? (
        <ExportDialog
          toolId={exportDialog.toolId}
          toolName={exportDialog.toolName || activeTool?.name || "Tool"}
          onClose={closeExportDialog}
        />
      ) : null}
      <CreateProjectDialog
        open={createProjectDialogOpen}
        busy={projectActionBusy}
        error={projectActionError}
        onClose={() => setCreateProjectDialogOpen(false)}
        onCreate={createProject}
      />
      <EditProjectDialog
        open={editProjectDialogOpen}
        project={editProject}
        busy={projectActionBusy}
        error={projectActionError}
        onClose={() => setEditProjectDialogOpen(null)}
        onSave={updateProject}
      />
      <AddChatsToProjectDialog
        open={addChatsDialogOpen}
        projectName={addChatsProject?.name ?? "Project"}
        conversations={conversations}
        assignedIds={
          new Set(
            projectConversations
              .filter((c) => c.projectId === addChatsProjectId)
              .map((c) => c.id),
          )
        }
        busy={projectActionBusy}
        error={projectActionError}
        onClose={() => setAddChatsDialogOpen(null)}
        onAssign={(ids) =>
          addChatsProjectId
            ? assignChats(addChatsProjectId, ids)
            : Promise.resolve()
        }
      />
      <DeleteProjectDialog
        open={deleteProjectDialogOpen}
        projectName={deleteProjectTarget?.name ?? "Project"}
        chatCount={
          conversations.filter((c) => c.projectId === deleteProjectId).length
        }
        busy={projectActionBusy}
        error={projectActionError}
        onClose={() => setDeleteProjectDialogOpen(null)}
        onDelete={(mode) =>
          deleteProjectId
            ? deleteProject(deleteProjectId, mode)
            : Promise.resolve()
        }
      />
      <RenameConversationDialog
        open={renameConversationDialogOpen}
        title={renameTarget?.title ?? ""}
        busy={projectActionBusy}
        error={projectActionError}
        onClose={() => setRenameConversationDialogOpen(null)}
        onRename={(title) =>
          renameConversationId
            ? renameConversation(renameConversationId, title)
            : Promise.resolve()
        }
      />
      <CommandPalette open={commandOpen} onClose={() => setCommandOpen(false)} />
      <PendingApprovalsHost />
      <WelcomeDialog />
      <TutorialOverlay />
      <ContextualEducationHost />
    </div>
  );
}
