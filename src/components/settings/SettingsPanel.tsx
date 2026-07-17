import { useState } from "react";
import { CheckCircle2, PlugZap, Trash2 } from "lucide-react";
import type { ThemePreference } from "@/types/agent";
import { useAppStore } from "@/stores/app-store";

export function SettingsPanel() {
  const {
    appInfo,
    aiStatus,
    theme,
    setTheme,
    testConnection,
    testingConnection,
    connectionTestMessage,
    clearConversations,
    clearTools,
    refreshAiStatus,
    setSettingsOpen,
  } = useAppStore();

  const [confirmClearChats, setConfirmClearChats] = useState(false);
  const [confirmClearTools, setConfirmClearTools] = useState(false);

  const themes: Array<{ id: ThemePreference; label: string }> = [
    { id: "system", label: "System" },
    { id: "light", label: "Light" },
    { id: "dark", label: "Dark" },
  ];

  const statusClass =
    aiStatus?.status === "ready"
      ? "ready"
      : aiStatus?.status === "missing_key" || aiStatus?.status === "unconfigured"
        ? "warn"
        : "";

  return (
    <section className="settings-panel" aria-label="Settings">
      <header className="settings-header">
        <div>
          <h1>Settings</h1>
          <p className="panel-subtitle" style={{ margin: 0 }}>
            Appearance, agent status, and local data
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={() => setSettingsOpen(false)}
        >
          Back to chat
        </button>
      </header>

      <div className="settings-content">
        <section className="settings-section" aria-labelledby="appearance-heading">
          <h2 id="appearance-heading">Appearance</h2>
          <p>Choose how Coreside looks on this device.</p>
          <div className="theme-options" role="group" aria-label="Theme">
            {themes.map((option) => (
              <button
                key={option.id}
                type="button"
                className="btn btn-secondary"
                aria-pressed={theme === option.id}
                onClick={() => void setTheme(option.id)}
              >
                {option.label}
              </button>
            ))}
          </div>
        </section>

        <section className="settings-section" aria-labelledby="ai-heading">
          <h2 id="ai-heading">AI agent</h2>
          <p>
            Credentials are loaded from your local <code>.env</code> file and
            never exposed to the interface.
          </p>
          <div className="button-row" style={{ marginBottom: "0.75rem" }}>
            <span className={`status-pill ${statusClass}`}>
              {aiStatus?.status === "ready" ? (
                <CheckCircle2 size={14} aria-hidden />
              ) : (
                <PlugZap size={14} aria-hidden />
              )}
              {aiStatus?.status ?? "unknown"}
            </span>
            <span className="muted">
              {aiStatus?.provider ?? "—"} · {aiStatus?.model ?? "—"}
            </span>
          </div>
          <p className="muted">{aiStatus?.message}</p>
          <div className="button-row">
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => void refreshAiStatus()}
            >
              Refresh status
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => void testConnection()}
              disabled={testingConnection}
            >
              {testingConnection ? "Testing…" : "Test connection"}
            </button>
          </div>
          {connectionTestMessage ? (
            <p role="status" className="muted">
              {connectionTestMessage}
            </p>
          ) : null}
        </section>

        <section className="settings-section" aria-labelledby="data-heading">
          <h2 id="data-heading">Data</h2>
          <p>
            Conversations, tools, and tool state are stored locally on this
            computer.
          </p>
          <div className="button-row">
            {!confirmClearChats ? (
              <button
                type="button"
                className="btn btn-danger"
                onClick={() => setConfirmClearChats(true)}
              >
                <Trash2 size={16} aria-hidden />
                Clear conversations
              </button>
            ) : (
              <>
                <button
                  type="button"
                  className="btn btn-danger"
                  onClick={() => {
                    void clearConversations();
                    setConfirmClearChats(false);
                  }}
                >
                  Confirm clear chats
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setConfirmClearChats(false)}
                >
                  Cancel
                </button>
              </>
            )}
          </div>
          <div className="button-row" style={{ marginTop: "0.75rem" }}>
            {!confirmClearTools ? (
              <button
                type="button"
                className="btn btn-danger"
                onClick={() => setConfirmClearTools(true)}
              >
                <Trash2 size={16} aria-hidden />
                Clear tools
              </button>
            ) : (
              <>
                <button
                  type="button"
                  className="btn btn-danger"
                  onClick={() => {
                    void clearTools();
                    setConfirmClearTools(false);
                  }}
                >
                  Confirm clear tools
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setConfirmClearTools(false)}
                >
                  Cancel
                </button>
              </>
            )}
          </div>
        </section>

        <section className="settings-section" aria-labelledby="about-heading">
          <h2 id="about-heading">About</h2>
          <p>
            <strong>{appInfo?.name ?? "Coreside"}</strong>{" "}
            <span className="muted">v{appInfo?.version ?? "0.1.0"}</span>
          </p>
          <p>
            {appInfo?.description ??
              "An AI-native personal software environment that can grow tools beside your conversations."}
          </p>
        </section>
      </div>
    </section>
  );
}
