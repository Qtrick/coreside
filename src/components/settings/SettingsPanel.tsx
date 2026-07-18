import { useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
import dockDarkUrl from "@/assets/branding/coreside-dock-dark.png";
import dockLightUrl from "@/assets/branding/coreside-dock-light.png";
import { CoresideLogo } from "@/components/branding/CoresideLogo";
import { AgentBehaviorSettings } from "@/components/settings/AgentBehaviorSettings";
import { AiProviderSettings } from "@/components/settings/AiProviderSettings";
import { WallpaperSettings } from "@/components/settings/WallpaperSettings";
import { api } from "@/lib/tauri";
import type { DockIconPreference, ThemePreference } from "@/types/agent";
import type { AddedSetting } from "@/types/settings";
import { useAppStore } from "@/stores/app-store";

export function SettingsPanel() {
  const {
    appInfo,
    theme,
    setTheme,
    dockIcon,
    setDockIcon,
    resolvedTheme,
    clearConversations,
    clearTools,
    navigateToChat,
  } = useAppStore();

  const [confirmClearChats, setConfirmClearChats] = useState(false);
  const [confirmClearTools, setConfirmClearTools] = useState(false);
  const [addedSettings, setAddedSettings] = useState<AddedSetting[]>([]);
  const [addedLoading, setAddedLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setAddedLoading(true);
    void api
      .listAddedSettings()
      .then((rows) => {
        if (!cancelled) setAddedSettings(rows);
      })
      .catch(() => {
        if (!cancelled) setAddedSettings([]);
      })
      .finally(() => {
        if (!cancelled) setAddedLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const themes: Array<{ id: ThemePreference; label: string }> = [
    { id: "system", label: "System" },
    { id: "light", label: "Light" },
    { id: "dark", label: "Dark" },
  ];

  const dockOptions: Array<{
    id: DockIconPreference;
    label: string;
    preview?: string;
  }> = [
    { id: "auto", label: "Auto" },
    { id: "dark", label: "Dark tile", preview: dockDarkUrl },
    { id: "light", label: "Light tile", preview: dockLightUrl },
  ];

  return (
    <section className="settings-panel" aria-label="Settings">
      <header className="settings-header">
        <div>
          <h1>Settings</h1>
          <p className="panel-subtitle" style={{ margin: 0 }}>
            Base Settings are product-owned. Added Settings come from your tools.
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={() => {
            const id = useAppStore.getState().activeConversationId;
            if (id) void navigateToChat(id);
            else useAppStore.setState({ view: { kind: "chat", conversationId: null } });
          }}
        >
          Back to chat
        </button>
      </header>

      <div className="settings-content">
        <div className="settings-group" aria-labelledby="base-settings-heading">
          <div className="settings-group-header">
            <h2 id="base-settings-heading" className="settings-group-title">
              Base Settings
            </h2>
            <p className="settings-group-desc">
              Branding logos and Base Settings structure stay protected. Theme,
              accents, backgrounds, borders, and text colors can be changed here
              or by asking the agent. Wallpaper templates live under Added
              Settings.
            </p>
          </div>

          <section className="settings-section" aria-labelledby="appearance-heading">
            <h3 id="appearance-heading">Appearance</h3>
            <p>
              Choose how Coreside looks inside the app window. Ask the agent for
              theme, accents, backgrounds, or borders. Wallpaper presets are
              under Templates in Added Settings.
            </p>
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

            <h4 className="settings-subheading" id="dock-icon-heading">
              Dock icon
            </h4>
            <p>
              macOS Dock tile. Auto follows system appearance; Dark and Light
              lock one of the two brand icons.
            </p>
            <div
              className="dock-icon-options"
              role="group"
              aria-labelledby="dock-icon-heading"
            >
              {dockOptions.map((option) => (
                <button
                  key={option.id}
                  type="button"
                  className="dock-icon-option"
                  aria-pressed={dockIcon === option.id}
                  onClick={() => void setDockIcon(option.id)}
                >
                  {option.preview ? (
                    <img
                      src={option.preview}
                      alt=""
                      className="dock-icon-preview"
                      width={56}
                      height={56}
                    />
                  ) : (
                    <span className="dock-icon-auto-badge" aria-hidden>
                      A
                    </span>
                  )}
                  <span>{option.label}</span>
                </button>
              ))}
            </div>
          </section>

          <AiProviderSettings />

          <AgentBehaviorSettings />

          <section className="settings-section" aria-labelledby="data-heading">
            <h3 id="data-heading">Data</h3>
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

          <section
            className="settings-section"
            aria-labelledby="accessibility-heading"
          >
            <h3 id="accessibility-heading">Accessibility</h3>
            <p>
              Coreside respects your system preference for reduced motion. When
              reduced motion is enabled, non-essential animations are minimized.
            </p>
            <p className="muted">
              Preference is read from{" "}
              <code>prefers-reduced-motion</code> — it is not overridden here.
            </p>
          </section>

          <section className="settings-section" aria-labelledby="about-heading">
            <h3 id="about-heading">About</h3>
            <div className="settings-about-brand">
              <CoresideLogo appearance={resolvedTheme} size={40} />
              <div>
                <p style={{ margin: 0 }}>
                  <strong>{appInfo?.name ?? "Coreside"}</strong>{" "}
                  <span className="muted">v{appInfo?.version ?? "0.1.0"}</span>
                </p>
                <p style={{ margin: "0.35rem 0 0" }}>
                  {appInfo?.description ??
                    "An AI-native personal software environment that can grow tools beside your conversations."}
                </p>
              </div>
            </div>
          </section>
        </div>

        <div className="settings-group" aria-labelledby="added-settings-heading">
          <div className="settings-group-header">
            <h2 id="added-settings-heading" className="settings-group-title">
              Added Settings
            </h2>
            <p className="settings-group-desc">
              Product template settings and preferences created for your personal
              tools.
            </p>
          </div>

          <section
            className="settings-section"
            aria-labelledby="templates-heading"
          >
            <h3 id="templates-heading">Templates</h3>
            <p>
              Workspace wallpaper presets and other product template settings.
            </p>
            <WallpaperSettings />
          </section>

          <section className="settings-section settings-section-added">
            {addedLoading ? (
              <p className="muted">Loading…</p>
            ) : addedSettings.length === 0 ? (
              <div className="settings-empty">
                <p>
                  Settings created for your personal tools will appear here.
                </p>
              </div>
            ) : (
              <ul className="added-settings-list">
                {addedSettings.map((setting) => (
                  <li key={setting.id} className="added-setting-item">
                    <div className="added-setting-label">{setting.label}</div>
                    {setting.description ? (
                      <p className="muted">{setting.description}</p>
                    ) : null}
                    <p className="added-setting-meta muted">
                      {setting.settingType}
                      {setting.ownerToolId
                        ? ` · tool ${setting.ownerToolId}`
                        : ""}
                    </p>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>
      </div>
    </section>
  );
}
