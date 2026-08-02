import { useEffect, useId, useMemo, useState } from "react";
import { Trash2 } from "lucide-react";
import dockDarkUrl from "@/assets/branding/coreside-dock-dark.png";
import dockLightUrl from "@/assets/branding/coreside-dock-light.png";
import { CoresideLogo } from "@/components/branding/CoresideLogo";
import { AgentBehaviorSettings } from "@/components/settings/AgentBehaviorSettings";
import { AiProviderSettings } from "@/components/settings/AiProviderSettings";
import { RecoverySettings } from "@/components/settings/RecoverySettings";
import { RuntimePermissionsSettings } from "@/components/applications/RuntimePermissionsSettings";
import { WallpaperSettings } from "@/components/settings/WallpaperSettings";
import { SearchSettingsSection } from "@/components/search/SearchProviderSetup";
import { api } from "@/lib/tauri";
import {
  matchSettingsSearch,
  readStoredSettingsCategory,
  SETTINGS_CATEGORIES,
  storeSettingsCategory,
  type SettingsCategoryId,
} from "@/lib/settings-categories";
import type { DockIconPreference, ThemePreference } from "@/types/agent";
import type { AddedSetting } from "@/types/settings";
import { useShallow } from "zustand/react/shallow";
import { useAppStore } from "@/stores/app-store";

function categorySummary(
  id: SettingsCategoryId,
  ctx: {
    theme: ThemePreference;
    adaptiveWindowSizing: string;
    aiReady: boolean;
    aiLabel: string;
    actionLogMode: string;
    addedCount: number;
  },
): string | null {
  switch (id) {
    case "general":
      return ctx.adaptiveWindowSizing === "off"
        ? "Window sizing off"
        : ctx.adaptiveWindowSizing === "ask"
          ? "Ask before expanding"
          : "Smart window sizing";
    case "appearance":
      return `Theme: ${ctx.theme}`;
    case "ai-access":
      return ctx.aiReady ? ctx.aiLabel : "Not connected";
    case "agent": {
      const modeLabel =
        ctx.actionLogMode === "always"
          ? "Always"
          : ctx.actionLogMode === "intelligent"
            ? "Intelligent"
            : "Off";
      return `Action Log: ${modeLabel}`;
    }
    case "search":
      return "Web research settings";
    case "privacy":
      return "Local diagnostics only by default";
    case "data":
      return "Stored on this computer";
    case "accessibility":
      return "Reduced motion follows system";
    case "advanced":
      return "Recovery and permissions";
    case "about":
      return null;
    case "added":
      return ctx.addedCount > 0
        ? `${ctx.addedCount} tool setting${ctx.addedCount === 1 ? "" : "s"}`
        : "Templates and tool settings";
  }
}

export function SettingsPanel() {
  const {
    appInfo,
    theme,
    setTheme,
    dockIcon,
    setDockIcon,
    resolvedTheme,
    adaptiveWindowSizing,
    setAdaptiveWindowSizing,
    clearConversations,
    clearTools,
    navigateToChat,
    aiStatus,
    actionLogMode,
  } = useAppStore(
    useShallow((s) => ({
      appInfo: s.appInfo,
      theme: s.theme,
      setTheme: s.setTheme,
      dockIcon: s.dockIcon,
      setDockIcon: s.setDockIcon,
      resolvedTheme: s.resolvedTheme,
      adaptiveWindowSizing: s.adaptiveWindowSizing,
      setAdaptiveWindowSizing: s.setAdaptiveWindowSizing,
      clearConversations: s.clearConversations,
      clearTools: s.clearTools,
      navigateToChat: s.navigateToChat,
      aiStatus: s.aiStatus,
      actionLogMode: s.actionLogMode,
    })),
  );

  const [category, setCategory] = useState<SettingsCategoryId>(() =>
    readStoredSettingsCategory(),
  );
  const [searchQuery, setSearchQuery] = useState("");
  const [confirmClearChats, setConfirmClearChats] = useState(false);
  const [confirmClearTools, setConfirmClearTools] = useState(false);
  const [addedSettings, setAddedSettings] = useState<AddedSetting[]>([]);
  const [addedLoading, setAddedLoading] = useState(true);
  const searchInputId = useId();
  const searchResultsId = useId();
  const navId = useId();
  const categoryHeadingId = useId();

  useEffect(() => {
    storeSettingsCategory(category);
  }, [category]);

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

  const searchHits = useMemo(
    () => matchSettingsSearch(searchQuery),
    [searchQuery],
  );

  const summaryCtx = {
    theme,
    adaptiveWindowSizing,
    aiReady: aiStatus?.status === "ready",
    aiLabel:
      aiStatus?.accessMode === "coreside_hosted"
        ? "Connected through Coreside AI"
        : aiStatus?.accessMode === "user_byok"
          ? "Connected through your provider"
          : aiStatus?.accessMode === "user_local"
            ? "Local AI ready"
            : "Ready for chat",
    actionLogMode,
    addedCount: addedSettings.length,
  };

  const active = SETTINGS_CATEGORIES.find((c) => c.id === category);
  const coresideCats = SETTINGS_CATEGORIES.filter((c) => c.group === "coreside");
  const addedCats = SETTINGS_CATEGORIES.filter((c) => c.group === "added");

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

  const selectCategory = (id: SettingsCategoryId) => {
    setCategory(id);
    setSearchQuery("");
    setConfirmClearChats(false);
    setConfirmClearTools(false);
  };

  return (
    <section className="settings-panel" aria-label="Settings">
      <header className="settings-header">
        <div>
          <h1>Settings</h1>
          <p className="panel-subtitle" style={{ margin: 0 }}>
            Coreside settings control the app itself. Tool settings are added by
            the tools you create.
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={() => {
            const id = useAppStore.getState().activeConversationId;
            if (id) void navigateToChat(id);
            else
              useAppStore.setState({
                view: { kind: "chat", conversationId: null },
              });
          }}
        >
          Back to chat
        </button>
      </header>

      <div className="settings-layout">
        <aside className="settings-nav" aria-labelledby={navId}>
          <h2 id={navId} className="visually-hidden">
            Settings categories
          </h2>
          <div className="settings-search">
            <label className="visually-hidden" htmlFor={searchInputId}>
              Search settings
            </label>
            <input
              id={searchInputId}
              type="search"
              className="settings-search-input"
              placeholder="Search settings"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape" && searchQuery) {
                  e.preventDefault();
                  setSearchQuery("");
                }
              }}
              autoComplete="off"
            />
            {searchHits.length > 0 ? (
              <ul
                id={searchResultsId}
                className="settings-search-results"
                aria-label="Matching settings"
              >
                {searchHits.map((hit) => (
                  <li key={hit.id}>
                    <button
                      type="button"
                      className="settings-search-hit"
                      onClick={() => selectCategory(hit.categoryId)}
                    >
                      <span className="settings-search-hit-label">
                        {hit.label}
                      </span>
                      <span className="muted settings-search-hit-cat">
                        {
                          SETTINGS_CATEGORIES.find(
                            (c) => c.id === hit.categoryId,
                          )?.label
                        }
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            ) : null}
            {searchQuery.trim() && searchHits.length === 0 ? (
              <p className="muted settings-search-empty" role="status">
                No matching settings.
              </p>
            ) : null}
          </div>

          <nav className="settings-nav-list" aria-label="Coreside settings">
            {coresideCats.map((cat) => {
              const summary = categorySummary(cat.id, summaryCtx);
              return (
                <button
                  key={cat.id}
                  type="button"
                  className="settings-nav-item"
                  aria-current={category === cat.id ? "page" : undefined}
                  onClick={() => selectCategory(cat.id)}
                >
                  <span className="settings-nav-item-label">{cat.label}</span>
                  {summary ? (
                    <span className="settings-nav-item-summary muted">
                      {summary}
                    </span>
                  ) : null}
                </button>
              );
            })}
          </nav>

          <nav className="settings-nav-list" aria-label="Added settings">
            {addedCats.map((cat) => {
              const summary = categorySummary(cat.id, summaryCtx);
              return (
                <button
                  key={cat.id}
                  type="button"
                  className="settings-nav-item"
                  aria-current={category === cat.id ? "page" : undefined}
                  onClick={() => selectCategory(cat.id)}
                >
                  <span className="settings-nav-item-label">{cat.label}</span>
                  {summary ? (
                    <span className="settings-nav-item-summary muted">
                      {summary}
                    </span>
                  ) : null}
                </button>
              );
            })}
          </nav>
        </aside>

        <div
          className="settings-content"
          role="region"
          aria-labelledby={categoryHeadingId}
        >
          <div className="settings-category-header">
            <h2 id={categoryHeadingId} className="settings-group-title">
              {active?.label ?? "Settings"}
            </h2>
            {active?.description ? (
              <p className="settings-group-desc">{active.description}</p>
            ) : null}
          </div>

          {category === "general" ? (
            <section className="settings-section" aria-labelledby="general-heading">
              <h3 id="general-heading" className="visually-hidden">
                General
              </h3>
              <h4 className="settings-subheading" id="adaptive-window-heading">
                Adaptive window sizing
              </h4>
              <p>
                Coreside may smoothly expand the current window when a tool needs
                more usable room, while remaining inside the current monitor.
              </p>
              <div
                className="theme-options"
                role="group"
                aria-labelledby="adaptive-window-heading"
              >
                {(
                  [
                    {
                      id: "smart" as const,
                      label: "Smart",
                      hint: "Expand automatically when needed",
                    },
                    {
                      id: "ask" as const,
                      label: "Ask first",
                      hint: "Confirm before expanding",
                    },
                    {
                      id: "off" as const,
                      label: "Off",
                      hint: "Never change the native window size",
                    },
                  ] as const
                ).map((option) => (
                  <button
                    key={option.id}
                    type="button"
                    className="btn btn-secondary"
                    aria-pressed={adaptiveWindowSizing === option.id}
                    title={option.hint}
                    onClick={() => void setAdaptiveWindowSizing(option.id)}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
              <button
                type="button"
                className="btn btn-ghost"
                style={{ marginTop: "0.5rem" }}
                onClick={() => void setAdaptiveWindowSizing("smart")}
              >
                Reset window behavior
              </button>
            </section>
          ) : null}

          {category === "appearance" ? (
            <section
              className="settings-section"
              aria-labelledby="appearance-heading"
            >
              <h3 id="appearance-heading">Theme</h3>
              <p>
                Choose how Coreside looks. For wallpapers and how much they show
                through panels, open Added Settings → Templates.
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
              <p className="muted" style={{ marginTop: "1rem" }}>
                <button
                  type="button"
                  className="btn btn-ghost"
                  onClick={() => selectCategory("added")}
                >
                  Open wallpaper templates
                </button>
              </p>
            </section>
          ) : null}

          {category === "ai-access" ? <AiProviderSettings /> : null}

          {category === "agent" ? <AgentBehaviorSettings /> : null}

          {category === "search" ? <SearchSettingsSection /> : null}

          {category === "privacy" ? (
            <section
              className="settings-section"
              aria-labelledby="privacy-heading"
            >
              <h3 id="privacy-heading">Privacy &amp; Security</h3>
              <p>
                Chats, tools, and most research caches stay on this computer.
                Provider keys use the operating system credential store when
                available. Coreside does not upload your chats for analytics by
                default.
              </p>
              <ul className="settings-plain-list">
                <li>Hosted AI (when enabled) only sends what that mode requires.</li>
                <li>BYOK keys never appear in tool state or exports.</li>
                <li>Action Log stores sanitized steps only when you enable it.</li>
              </ul>
              <div className="button-row">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => selectCategory("advanced")}
                >
                  Manage application permissions
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => selectCategory("search")}
                >
                  Research cache controls
                </button>
              </div>
            </section>
          ) : null}

          {category === "data" ? (
            <section className="settings-section" aria-labelledby="data-heading">
              <h3 id="data-heading">Data &amp; Storage</h3>
              <p>
                Conversations, tools, and tool state are stored locally on this
                computer. Clearing data cannot be undone from this screen.
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
              <p className="muted" style={{ marginTop: "1rem" }}>
                Full backup and restore for public beta is tracked separately;
                Recovery Mode can restore last working tool versions.
              </p>
              <button
                type="button"
                className="btn btn-ghost"
                onClick={() => selectCategory("advanced")}
              >
                Open Recovery
              </button>
            </section>
          ) : null}

          {category === "accessibility" ? (
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
                Preference is read from the system — it is not overridden here.
              </p>
            </section>
          ) : null}

          {category === "advanced" ? (
            <div className="settings-group">
              <RecoverySettings />
              <RuntimePermissionsSettings />
            </div>
          ) : null}

          {category === "about" ? (
            <section className="settings-section" aria-labelledby="about-heading">
              <h3 id="about-heading">About</h3>
              <div className="settings-about-brand">
                <CoresideLogo appearance={resolvedTheme} size={40} />
                <div>
                  <p style={{ margin: 0 }}>
                    <strong>{appInfo?.name ?? "Coreside"}</strong>{" "}
                    <span className="muted">
                      v{appInfo?.version ?? "0.1.0"}
                    </span>
                  </p>
                  <p style={{ margin: "0.35rem 0 0" }}>
                    {appInfo?.description ??
                      "An AI-native personal software environment that can grow tools beside your conversations."}
                  </p>
                </div>
              </div>
            </section>
          ) : null}

          {category === "added" ? (
            <div className="settings-group">
              <section
                className="settings-section"
                aria-labelledby="templates-heading"
              >
                <h3 id="templates-heading">Templates</h3>
                <p>
                  Workspace wallpaper presets and other product template
                  settings.
                </p>
                <WallpaperSettings />
              </section>

              <section className="settings-section settings-section-added">
                <h3>Tool settings</h3>
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
                        <div className="added-setting-label">
                          {setting.label}
                        </div>
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
          ) : null}
        </div>
      </div>
    </section>
  );
}
