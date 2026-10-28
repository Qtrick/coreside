type AppAppearancePreviewProps = {
  mode: "light" | "dark";
};

/**
 * Lightweight, presentational preview of the Coreside desktop environment.
 * Renders miniature window chrome, sidebar, chat, composer, and generated application panel.
 * Uses zero external stores or side effects.
 */
export function AppAppearancePreview({ mode }: AppAppearancePreviewProps) {
  const isDark = mode === "dark";

  return (
    <div
      className={`app-appearance-preview ${isDark ? "preview-dark" : "preview-light"}`}
      aria-hidden="true"
    >
      {/* Mini Titlebar */}
      <div className="preview-titlebar">
        <div className="preview-window-dots">
          <span className="dot dot-close" />
          <span className="dot dot-min" />
          <span className="dot dot-zoom" />
        </div>
        <div className="preview-app-title">Coreside</div>
      </div>

      {/* Mini Main Workspace */}
      <div className="preview-workspace">
        {/* Mini Sidebar */}
        <div className="preview-sidebar">
          <div className="preview-sidebar-item active">
            <span className="preview-icon" />
            <span className="preview-nav-line w-60" />
          </div>
          <div className="preview-sidebar-item">
            <span className="preview-icon" />
            <span className="preview-nav-line w-40" />
          </div>
          <div className="preview-sidebar-item">
            <span className="preview-icon" />
            <span className="preview-nav-line w-50" />
          </div>
        </div>

        {/* Mini Chat Column */}
        <div className="preview-chat-column">
          <div className="preview-chat-header">
            <span className="preview-header-line" />
          </div>
          <div className="preview-messages">
            <div className="preview-msg preview-msg-user">
              <span className="msg-line w-80" />
            </div>
            <div className="preview-msg preview-msg-assistant">
              <span className="msg-line w-90" />
              <span className="msg-line w-65" />
            </div>
          </div>
          <div className="preview-composer">
            <span className="composer-placeholder" />
            <span className="composer-send-btn" />
          </div>
        </div>

        {/* Mini Generated Application Panel */}
        <div className="preview-app-panel">
          <div className="preview-app-header">
            <span className="app-title-line" />
            <span className="app-badge" />
          </div>
          <div className="preview-app-content">
            <div className="preview-app-card">
              <span className="card-line w-75" />
              <span className="card-line w-50 muted" />
            </div>
            <div className="preview-app-card">
              <span className="card-line w-60" />
              <span className="card-line w-40 muted" />
            </div>
          </div>
          <div className="preview-app-actions">
            <span className="preview-btn preview-btn-secondary" />
            <span className="preview-btn preview-btn-primary" />
          </div>
        </div>
      </div>
    </div>
  );
}
