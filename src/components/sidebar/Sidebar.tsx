import {
  ChevronLeft,
  ChevronRight,
  MessageSquarePlus,
  Settings,
  Wrench,
} from "lucide-react";
import { useAppStore } from "@/stores/app-store";

export function Sidebar() {
  const {
    sidebarCollapsed,
    toggleSidebar,
    conversations,
    activeConversationId,
    selectConversation,
    createConversation,
    tools,
    activeToolId,
    selectTool,
    settingsOpen,
    setSettingsOpen,
  } = useAppStore();

  return (
    <aside
      className={`sidebar${sidebarCollapsed ? " collapsed" : ""}`}
      aria-label="Coreside sidebar"
    >
      <div className="sidebar-brand">
        <button
          type="button"
          className="wordmark"
          onClick={() => setSettingsOpen(false)}
          aria-label="Coreside home"
        >
          <span className="wordmark-mark" aria-hidden />
          <span className="wordmark-text">Coreside</span>
        </button>
        <button
          type="button"
          className="icon-btn"
          onClick={() => void toggleSidebar()}
          aria-label={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          aria-expanded={!sidebarCollapsed}
        >
          {sidebarCollapsed ? <ChevronRight size={18} /> : <ChevronLeft size={18} />}
        </button>
      </div>

      <button
        type="button"
        className="btn btn-primary btn-block"
        onClick={() => void createConversation()}
        aria-label="New chat"
      >
        <MessageSquarePlus size={16} aria-hidden />
        <span className="nav-label">New chat</span>
      </button>

      <section className="sidebar-section grow" aria-label="Recent chats">
        <div className="sidebar-section-label">Chats</div>
        <div className="sidebar-nav">
          {conversations.length === 0 ? (
            <p className="muted" style={{ padding: "0.5rem 0.75rem", margin: 0 }}>
              <span className="nav-label">No chats yet</span>
            </p>
          ) : (
            conversations.map((conversation) => (
              <button
                key={conversation.id}
                type="button"
                className={
                  !settingsOpen && activeConversationId === conversation.id
                    ? "active"
                    : undefined
                }
                onClick={() => void selectConversation(conversation.id)}
              >
                <span className="nav-label">{conversation.title}</span>
              </button>
            ))
          )}
        </div>
      </section>

      <section className="sidebar-section grow" aria-label="Personal tools">
        <div className="sidebar-section-label">Tools</div>
        <div className="sidebar-nav">
          {tools.length === 0 ? (
            <p className="muted" style={{ padding: "0.5rem 0.75rem", margin: 0 }}>
              <span className="nav-label">No tools yet</span>
            </p>
          ) : (
            tools.map((tool) => (
              <button
                key={tool.id}
                type="button"
                className={
                  !settingsOpen && activeToolId === tool.id ? "active" : undefined
                }
                onClick={() => void selectTool(tool.id)}
              >
                <Wrench size={16} aria-hidden />
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
          className={settingsOpen ? "active" : undefined}
          style={{
            display: "flex",
            alignItems: "center",
            gap: "0.75rem",
            width: "100%",
            textAlign: "left",
            padding: "0.65rem 0.75rem",
            borderRadius: "8px",
            border: "1px solid transparent",
            background: settingsOpen
              ? "color-mix(in srgb, var(--accent-primary) 14%, var(--surface))"
              : "transparent",
          }}
          onClick={() => setSettingsOpen(true)}
          aria-label="Settings"
        >
          <Settings size={16} aria-hidden />
          <span className="nav-label">Settings</span>
        </button>
      </div>
    </aside>
  );
}
