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
import { resolveActiveWallpaper } from "@/lib/wallpaper";
import { wallpaperDataAttribute } from "@/types/wallpaper";
import { CreateProjectDialog } from "@/components/projects/CreateProjectDialog";
import { EditProjectDialog } from "@/components/projects/EditProjectDialog";
import { AddChatsToProjectDialog } from "@/components/projects/AddChatsToProjectDialog";
import { DeleteProjectDialog } from "@/components/projects/DeleteProjectDialog";
import { RenameConversationDialog } from "@/components/projects/RenameConversationDialog";
import { useAppStore } from "@/stores/app-store";

export function AppShell() {
  const sidebarCollapsed = useAppStore((s) => s.sidebarCollapsed);
  const view = useAppStore((s) => s.view);
  const activeToolId = useAppStore((s) => s.activeToolId);
  const activeTool = useAppStore((s) => s.activeTool);
  const exportDialog = useAppStore((s) => s.exportDialog);
  const closeExportDialog = useAppStore((s) => s.closeExportDialog);
  const wallpaper = useAppStore((s) => s.wallpaper);
  const globalWallpaperJson = useAppStore((s) => s.globalWallpaperJson);
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

  const classes = [
    "app-shell",
    sidebarCollapsed ? "sidebar-collapsed" : "",
    !showToolCanvas ? "no-tool" : "",
    overlayMode ? "settings-mode" : "",
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
              window.alert("Project context search will be available in a later phase.");
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
        return (
          <>
            <ChatPanel />
            {showToolCanvas ? <ToolCanvas /> : null}
          </>
        );
    }
  })();

  return (
    <div
      className={classes}
      data-wallpaper={wallpaperAttr}
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
    </div>
  );
}
