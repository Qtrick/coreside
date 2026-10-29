import {
  Columns2,
  Columns3,
  Grid2X2,
  MessageSquare,
  Plus,
  Rows2,
  Square,
  Wrench,
  Folder,
  Image,
  RefreshCw,
  Settings,
  X,
} from "lucide-react";
import { useWorkspaceStore } from "@/stores/workspace-store";
import { useAppStore } from "@/stores/app-store";
import { useShallow } from "zustand/react/shallow";
import type { WorkspaceTabKind } from "@/types/workspace";

function getTabIcon(kind: WorkspaceTabKind) {
  switch (kind) {
    case "chat":
      return <MessageSquare size={13} aria-hidden />;
    case "app":
      return <Wrench size={13} aria-hidden />;
    case "project":
    case "projects":
      return <Folder size={13} aria-hidden />;
    case "media":
      return <Image size={13} aria-hidden />;
    case "automations":
      return <RefreshCw size={13} aria-hidden />;
    case "settings":
      return <Settings size={13} aria-hidden />;
    default:
      return <MessageSquare size={13} aria-hidden />;
  }
}

interface WorkspaceTabStripProps {
  paneId: string;
}

export function WorkspaceTabStrip({ paneId }: WorkspaceTabStripProps) {
  const {
    panes,
    tabs,
    layoutKind,
    setLayout,
    activateTab,
    closeTab,
    openTab,
    reorderTabs,
    setFocusedPane,
  } = useWorkspaceStore(
    useShallow((s) => ({
      panes: s.panes,
      tabs: s.tabs,
      layoutKind: s.layoutKind,
      setLayout: s.setLayout,
      activateTab: s.activateTab,
      closeTab: s.closeTab,
      openTab: s.openTab,
      reorderTabs: s.reorderTabs,
      setFocusedPane: s.setFocusedPane,
    })),
  );

  const currentPane = panes.find((p) => p.id === paneId) || panes[0];
  const tabIds = currentPane?.tabIds || [];
  const activeTabId = currentPane?.activeTabId;

  const handleNewTab = () => {
    openTab(
      {
        kind: "chat",
        title: "New chat",
        conversationId: null,
      },
      paneId,
    );
    setFocusedPane(paneId);
    useAppStore.setState({
      view: { kind: "chat", conversationId: null },
      activeConversationId: null,
    });
  };

  const handleTabClick = (tabId: string) => {
    activateTab(tabId, paneId);
    setFocusedPane(paneId);
    const tab = tabs[tabId];
    if (tab) {
      if (tab.kind === "chat") {
        if (tab.conversationId) {
          void useAppStore.getState().navigateToChat(tab.conversationId);
        } else {
          useAppStore.setState({
            view: { kind: "chat", conversationId: null },
            activeConversationId: null,
          });
        }
      } else if (tab.kind === "app" && tab.toolId) {
        void useAppStore.getState().selectTool(tab.toolId);
      } else if (tab.kind === "project" && tab.projectId) {
        void useAppStore.getState().navigateToProject(tab.projectId);
      }
    }
  };

  const handleCloseTab = (e: React.MouseEvent, tabId: string) => {
    e.stopPropagation();
    closeTab(tabId, paneId);
  };

  return (
    <div
      className="workspace-tab-strip"
      role="tablist"
      aria-label="Workspace tabs"
      data-testid="workspace-tab-strip"
    >
      <div className="workspace-tabs-container">
        {tabIds.map((tabId, index) => {
          const tab = tabs[tabId];
          if (!tab) return null;
          const isActive = tabId === activeTabId;
          return (
            <div
              key={tabId}
              role="tab"
              aria-selected={isActive}
              className={`workspace-tab${isActive ? " active" : ""}`}
              onClick={() => handleTabClick(tabId)}
              title={tab.title}
              tabIndex={isActive ? 0 : -1}
              draggable={true}
              onDragStart={(e) => {
                e.dataTransfer.setData("text/plain", tabId);
              }}
              onDragOver={(e) => {
                e.preventDefault();
              }}
              onDrop={(e) => {
                e.preventDefault();
                const draggedId = e.dataTransfer.getData("text/plain");
                if (draggedId && draggedId !== tabId) {
                  const fromIdx = tabIds.indexOf(draggedId);
                  const toIdx = tabIds.indexOf(tabId);
                  if (fromIdx >= 0 && toIdx >= 0) {
                    reorderTabs(paneId, fromIdx, toIdx);
                  }
                }
              }}
              onKeyDown={(e) => {
                if (e.altKey && e.key === "ArrowLeft") {
                  e.preventDefault();
                  if (index > 0) {
                    reorderTabs(paneId, index, index - 1);
                  }
                } else if (e.altKey && e.key === "ArrowRight") {
                  e.preventDefault();
                  if (index < tabIds.length - 1) {
                    reorderTabs(paneId, index, index + 1);
                  }
                }
              }}
            >
              <span className="workspace-tab-icon">{getTabIcon(tab.kind)}</span>
              <span className="workspace-tab-title">{tab.title}</span>
              {tab.closable !== false ? (
                <button
                  type="button"
                  className="workspace-tab-close"
                  onClick={(e) => handleCloseTab(e, tabId)}
                  aria-label={`Close ${tab.title} tab`}
                  title="Close tab"
                >
                  <X size={12} />
                </button>
              ) : null}
            </div>
          );
        })}

        {/* Plus new tab button */}
        <button
          type="button"
          className="workspace-tab-add"
          onClick={handleNewTab}
          aria-label="New tab"
          title="New tab (Cmd+T)"
        >
          <Plus size={14} />
        </button>
      </div>

      {/* Split View Layout Controls (Visible on header of primary / top tab strip) */}
      <div
        className="workspace-split-controls"
        role="group"
        aria-label="Split view layout controls"
      >
        {/* 1 Pane */}
        <button
          type="button"
          className={`split-btn${layoutKind === "single" ? " active" : ""}`}
          onClick={() => setLayout("single")}
          aria-label="Single pane view"
          title="Single view"
          aria-pressed={layoutKind === "single"}
        >
          <Square size={14} />
        </button>

        {/* 2 Panes Vertical */}
        <button
          type="button"
          className={`split-btn${layoutKind === "split-vertical" ? " active" : ""}`}
          onClick={() => setLayout("split-vertical")}
          aria-label="Split view vertical (2 panes)"
          title="Split view (up to 4)"
          aria-pressed={layoutKind === "split-vertical"}
        >
          <Columns2 size={14} />
        </button>

        {/* 2 Panes Horizontal */}
        <button
          type="button"
          className={`split-btn${layoutKind === "split-horizontal" ? " active" : ""}`}
          onClick={() => setLayout("split-horizontal")}
          aria-label="Split view horizontal (2 panes)"
          title="Horizontal split (2 panes)"
          aria-pressed={layoutKind === "split-horizontal"}
        >
          <Rows2 size={14} />
        </button>

        {/* 3 Panes */}
        <button
          type="button"
          className={`split-btn${layoutKind === "split-three" ? " active" : ""}`}
          onClick={() => setLayout("split-three")}
          aria-label="Split view (3 panes)"
          title="3-pane layout"
          aria-pressed={layoutKind === "split-three"}
        >
          <Columns3 size={14} />
        </button>

        {/* 4 Panes Grid */}
        <button
          type="button"
          className={`split-btn${layoutKind === "split-grid" ? " active" : ""}`}
          onClick={() => setLayout("split-grid")}
          aria-label="Split view grid (4 panes)"
          title="4-pane grid"
          aria-pressed={layoutKind === "split-grid"}
        >
          <Grid2X2 size={14} />
        </button>
      </div>
    </div>
  );
}
