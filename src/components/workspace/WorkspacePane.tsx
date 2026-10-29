import { useWorkspaceStore } from "@/stores/workspace-store";
import { useAppStore } from "@/stores/app-store";
import { ChatPanel } from "@/components/chat/ChatPanel";
import { ToolCanvas } from "@/components/tool-canvas/ToolCanvas";
import { WorkspaceDashboard } from "./WorkspaceDashboard";
import { SettingsPanel } from "@/components/settings/SettingsPanel";
import { AutomationsPanel } from "@/components/automations/AutomationsPanel";
import { MediaLibrary } from "@/components/media/MediaLibrary";
import { ProjectPage } from "@/components/projects/ProjectPage";
import { ProjectsListPage } from "@/components/projects/ProjectsListPage";

interface WorkspacePaneProps {
  paneId: string;
  isFocused: boolean;
  onFocus: () => void;
}

export function WorkspacePane({
  paneId,
  isFocused,
  onFocus,
}: WorkspacePaneProps) {
  const pane = useWorkspaceStore((s) => s.panes.find((p) => p.id === paneId));
  const tabs = useWorkspaceStore((s) => s.tabs);
  const activeTab = pane?.activeTabId ? tabs[pane.activeTabId] : null;

  const projects = useAppStore((s) => s.projects);
  const activeProject = useAppStore((s) => s.activeProject);
  const projectConversations = useAppStore((s) => s.projectConversations);
  const navigateToProjects = useAppStore((s) => s.navigateToProjects);
  const navigateToProject = useAppStore((s) => s.navigateToProject);
  const navigateToChat = useAppStore((s) => s.navigateToChat);
  const createChatInProject = useAppStore((s) => s.createChatInProject);
  const setCreateProjectDialogOpen = useAppStore((s) => s.setCreateProjectDialogOpen);
  const setEditProjectDialogOpen = useAppStore((s) => s.setEditProjectDialogOpen);
  const setManageContextDialogOpen = useAppStore((s) => s.setManageContextDialogOpen);
  const setAddChatsDialogOpen = useAppStore((s) => s.setAddChatsDialogOpen);

  const renderContent = () => {
    if (!activeTab) {
      return <WorkspaceDashboard autoFocus={isFocused} />;
    }

    switch (activeTab.kind) {
      case "chat":
        if (!activeTab.conversationId) {
          return <WorkspaceDashboard autoFocus={isFocused} />;
        }
        return (
          <ChatPanel
            conversationId={activeTab.conversationId}
            autoFocus={isFocused}
          />
        );

      case "app":
        return <ToolCanvas toolId={activeTab.toolId} />;

      case "project": {
        const targetProj =
          projects.find((p) => p.id === activeTab.projectId) ?? activeProject;
        if (!targetProj) {
          return (
            <ProjectsListPage
              projects={projects}
              onCreateProject={() => setCreateProjectDialogOpen(true)}
              onOpenProject={(id) => void navigateToProject(id)}
            />
          );
        }
        return (
          <ProjectPage
            project={targetProj}
            conversations={projectConversations}
            onBack={() => navigateToProjects()}
            onNewChat={() => void createChatInProject(targetProj.id)}
            onOpenChat={(id) => void navigateToChat(id)}
            onEditProject={() => setEditProjectDialogOpen(targetProj.id)}
            onManageContext={() => setManageContextDialogOpen(targetProj.id)}
            onAddChats={() => setAddChatsDialogOpen(targetProj.id)}
          />
        );
      }

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

      case "settings":
        return <SettingsPanel />;

      default:
        return <WorkspaceDashboard />;
    }
  };

  const handleFocus = () => {
    onFocus();
    if (activeTab) {
      if (activeTab.kind === "chat" && activeTab.conversationId) {
        if (useAppStore.getState().activeConversationId !== activeTab.conversationId) {
          void useAppStore.getState().navigateToChat(activeTab.conversationId);
        }
      } else if (activeTab.kind === "app" && activeTab.toolId) {
        if (useAppStore.getState().activeToolId !== activeTab.toolId) {
          void useAppStore.getState().selectTool(activeTab.toolId);
        }
      } else if (activeTab.kind === "project" && activeTab.projectId) {
        if (useAppStore.getState().activeProjectId !== activeTab.projectId) {
          void useAppStore.getState().navigateToProject(activeTab.projectId);
        }
      }
    }
  };

  return (
    <div
      className={`workspace-pane${isFocused ? " focused" : ""}`}
      data-pane-id={paneId}
      onClick={handleFocus}
      onFocusCapture={handleFocus}
      tabIndex={-1}
      role="region"
      aria-label={`Workspace pane ${paneId}`}
    >
      {renderContent()}
    </div>
  );
}

