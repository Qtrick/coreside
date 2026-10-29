import { useEffect } from "react";
import { useWorkspaceStore } from "@/stores/workspace-store";
import { WorkspaceTabStrip } from "./WorkspaceTabStrip";
import { WorkspaceGrid } from "./WorkspaceGrid";
import { useAppStore } from "@/stores/app-store";

export function WorkspaceView() {
  const focusedPaneId = useWorkspaceStore((s) => s.focusedPaneId);
  const closeTab = useWorkspaceStore((s) => s.closeTab);
  const openTab = useWorkspaceStore((s) => s.openTab);
  const setLayout = useWorkspaceStore((s) => s.setLayout);
  const panes = useWorkspaceStore((s) => s.panes);
  const tabs = useWorkspaceStore((s) => s.tabs);
  const setFocusedPane = useWorkspaceStore((s) => s.setFocusedPane);
  const reconcileWithAvailableResources = useWorkspaceStore(
    (s) => s.reconcileWithAvailableResources,
  );

  const conversations = useAppStore((s) => s.conversations);
  const tools = useAppStore((s) => s.tools);
  const projects = useAppStore((s) => s.projects);
  const activeConversationId = useAppStore((s) => s.activeConversationId);
  const activeToolId = useAppStore((s) => s.activeToolId);
  const bootstrapped = useAppStore((s) => s.bootstrapped);

  // Sync workspace tabs when active conversation in app-store changes
  useEffect(() => {
    if (activeConversationId) {
      const activeConv = conversations.find((c) => c.id === activeConversationId);
      if (activeConv) {
        openTab(
          {
            kind: "chat",
            title: activeConv.title,
            conversationId: activeConv.id,
          },
          focusedPaneId,
        );
      }
    }
  }, [activeConversationId, conversations, focusedPaneId, openTab]);

  // Sync workspace tabs when active tool in app-store changes
  useEffect(() => {
    if (activeToolId) {
      const activeTool = tools.find((t) => t.id === activeToolId);
      if (activeTool) {
        openTab(
          {
            kind: "app",
            title: activeTool.name,
            toolId: activeTool.id,
          },
          focusedPaneId,
        );
      }
    }
  }, [activeToolId, tools, focusedPaneId, openTab]);

  // Reconcile deleted resources periodically (only after initial bootstrap completes)
  useEffect(() => {
    if (!bootstrapped) return;
    reconcileWithAvailableResources({
      conversationIds: new Set(conversations.map((c) => c.id)),
      toolIds: new Set(tools.map((t) => t.id)),
      projectIds: new Set(projects.map((p) => p.id)),
    });
  }, [bootstrapped, conversations, tools, projects, reconcileWithAvailableResources]);

  // Keyboard shortcuts for tabs and splits
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const isCmd = e.metaKey || e.ctrlKey;
      if (!isCmd) return;

      // Cmd+T: New Tab
      if (e.key.toLowerCase() === "t" && !e.shiftKey) {
        e.preventDefault();
        openTab(
          {
            kind: "chat",
            title: "New chat",
            conversationId: null,
          },
          focusedPaneId,
        );
      }
      // Cmd+W: Close current tab
      else if (e.key.toLowerCase() === "w") {
        e.preventDefault();
        const currentPane = panes.find((p) => p.id === focusedPaneId);
        if (currentPane?.activeTabId) {
          closeTab(currentPane.activeTabId, focusedPaneId);
        }
      }
      // Cmd+Alt+1..4 & Cmd+Alt+H/5: Quick Split switch
      else if (e.altKey && e.key === "1") {
        e.preventDefault();
        setLayout("single");
      } else if (e.altKey && e.key === "2") {
        e.preventDefault();
        setLayout("split-vertical");
      } else if (e.altKey && e.key === "3") {
        e.preventDefault();
        setLayout("split-three");
      } else if (e.altKey && e.key === "4") {
        e.preventDefault();
        setLayout("split-grid");
      } else if (e.altKey && (e.key === "5" || e.key.toLowerCase() === "h")) {
        e.preventDefault();
        setLayout("split-horizontal");
      }
      // Cmd+Alt+ArrowLeft / ArrowRight: Switch pane focus
      else if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
        e.preventDefault();
        const currentIdx = panes.findIndex((p) => p.id === focusedPaneId);
        if (currentIdx >= 0) {
          const nextIdx =
            e.key === "ArrowRight"
              ? (currentIdx + 1) % panes.length
              : (currentIdx - 1 + panes.length) % panes.length;
          setFocusedPane(panes[nextIdx].id);
        }
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [focusedPaneId, panes, tabs, openTab, closeTab, setLayout, setFocusedPane]);

  return (
    <div
      className="workspace-view"
      data-testid="workspace-view"
    >
      <WorkspaceTabStrip paneId={focusedPaneId} />
      <WorkspaceGrid />
    </div>
  );
}

