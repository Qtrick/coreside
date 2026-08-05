import { useEffect, useId, useMemo, useRef, useState } from "react";
import { Search, Trash2, X } from "lucide-react";
import dockDarkUrl from "@/assets/branding/coreside-dock-dark.png";
import dockLightUrl from "@/assets/branding/coreside-dock-light.png";
import { CoresideLogo } from "@/components/branding/CoresideLogo";
import { AgentBehaviorSettings } from "@/components/settings/AgentBehaviorSettings";
import { AiProviderSettings } from "@/components/settings/AiProviderSettings";
import { RecoverySettings } from "@/components/settings/RecoverySettings";
import { RuntimePermissionsSettings } from "@/components/applications/RuntimePermissionsSettings";
import { WallpaperSettings } from "@/components/settings/WallpaperSettings";
import { SearchSettingsSection } from "@/components/search/SearchProviderSetup";
import { listConsumerTutorials, ESSENTIALS_TUTORIAL_ID } from "@/lib/onboarding/tutorials";
import {
  WHATS_NEW_ITEMS,
  WHATS_NEW_VERSION_LABEL,
} from "@/lib/onboarding/whats-new";
import { useOnboardingStore } from "@/stores/onboarding-store";
import { api } from "@/lib/tauri";
import {
  matchSettingsSearch,
  normalizeSettingsCategoryId,
  readStoredSettingsCategory,
  SETTINGS_CATEGORIES,
  settingsTargetDomId,
  storeSettingsCategory,
  type SettingsCategoryId,
  type SettingsSearchHit,
} from "@/lib/settings-categories";
import type { DockIconPreference, ThemePreference } from "@/types/agent";
import type { AddedSetting } from "@/types/settings";
import { useShallow } from "zustand/react/shallow";
import { usePresence } from "@/lib/motion/usePresence";
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
      return `Activity: ${modeLabel}`;
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
    case "help-learning":
      return "Tours and guides";
    case "about":
      return null;
    case "added":
      return ctx.addedCount > 0
        ? `${ctx.addedCount} app setting${ctx.addedCount === 1 ? "" : "s"}`
        : "Preferences from your apps";
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
  const [displayCategory, setDisplayCategory] = useState(category);
  const paneStable = category === displayCategory;
  const {
    mounted: paneMounted,
    phase: panePhase,
    onEnterComplete: onPaneEnterComplete,
    onExitComplete: onPaneExitComplete,
    reducedMotion: paneReducedMotion,
  } = usePresence(paneStable);
  const [searchQuery, setSearchQuery] = useState("");
  const [pendingFocusTarget, setPendingFocusTarget] = useState<string | null>(
    null,
  );
  const [confirmClearChats, setConfirmClearChats] = useState(false);
  const [confirmClearTools, setConfirmClearTools] = useState(false);
  const [addedSettings, setAddedSettings] = useState<AddedSetting[]>([]);
  const [addedLoading, setAddedLoading] = useState(true);
  const [storageSummary, setStorageSummary] = useState<{
    chatCount: number;
    toolCount: number;
    projectCount: number;
    databaseBytes: number | null;
    backupBytes: number | null;
    mediaBytes: number | null;
    profileReady: boolean;
  } | null>(null);
  const [storageLoading, setStorageLoading] = useState(false);
  const [dbHealth, setDbHealth] = useState<{
    status: string;
    quickCheck: string;
    foreignKeyCheck: string;
    schemaVersion: string | null;
  } | null>(null);
  const [backupBusy, setBackupBusy] = useState(false);
  const [backupMessage, setBackupMessage] = useState<string | null>(null);
  const searchInputId = useId();
  const searchResultsId = useId();
  const searchStatusId = useId();
  const navId = useId();
  const categoryHeadingId = useId();
  const searchInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    storeSettingsCategory(category);
  }, [category]);

  useEffect(() => {
    if (category !== displayCategory && panePhase === "exited") {
      setDisplayCategory(category);
    }
  }, [category, displayCategory, panePhase]);

  useEffect(() => {
    const onExternalCategory = (event: Event) => {
      const detail = (event as CustomEvent<unknown>).detail;
      setCategory((prev) => normalizeSettingsCategoryId(detail, prev));
    };
    window.addEventListener("coreside:settings-category", onExternalCategory);
    return () => {
      window.removeEventListener(
        "coreside:settings-category",
        onExternalCategory,
      );
    };
  }, []);

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

  useEffect(() => {
    if (category !== "data" && category !== "privacy") return;
    let cancelled = false;
    setStorageLoading(true);
    void api
      .getStorageSummary()
      .then((summary) => {
        if (!cancelled) setStorageSummary(summary);
      })
      .catch(() => {
        if (!cancelled) setStorageSummary(null);
      })
      .finally(() => {
        if (!cancelled) setStorageLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [category]);

  const searchHits = useMemo(
    () => matchSettingsSearch(searchQuery),
    [searchQuery],
  );
  const searchActive = searchQuery.trim().length > 0;

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

  const developerMode = useAppStore((s) => s.developerMode);
  const startEssentials = useOnboardingStore((s) => s.startEssentials);
  const startModule = useOnboardingStore((s) => s.startModule);
  const resetEssentials = useOnboardingStore((s) => s.resetEssentials);
  const onboardingProgress = useOnboardingStore((s) => s.progressById);
  const essentialsProgress = onboardingProgress[ESSENTIALS_TUTORIAL_ID];
  const unfinishedEssentials =
    essentialsProgress?.status === "in_progress";
  const active = SETTINGS_CATEGORIES.find((c) => c.id === displayCategory);
  const categoryTransitioning = category !== displayCategory || panePhase !== "entered";
  const coresideCats = SETTINGS_CATEGORIES.filter((c) => c.group === "coreside");
  const addedCats = SETTINGS_CATEGORIES.filter((c) => c.group === "added");

  useEffect(() => {
    if (!pendingFocusTarget || categoryTransitioning) return;
    const el = document.getElementById(pendingFocusTarget);
    if (!el) {
      setPendingFocusTarget(null);
      return;
    }
    el.scrollIntoView({ block: "nearest", behavior: "smooth" });
    if (el instanceof HTMLElement) {
      if (!el.hasAttribute("tabindex")) el.tabIndex = -1;
      el.focus({ preventScroll: true });
    }
    setPendingFocusTarget(null);
  }, [pendingFocusTarget, categoryTransitioning, displayCategory]);

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

  const selectSearchHit = (hit: SettingsSearchHit) => {
    const targetId = hit.id.startsWith("category-")
      ? categoryHeadingId
      : settingsTargetDomId(hit.id);
    setPendingFocusTarget(targetId);
    setCategory(hit.categoryId);
    setSearchQuery("");
    setConfirmClearChats(false);
    setConfirmClearTools(false);
  };

  const clearSearch = () => {
    setSearchQuery("");
    searchInputRef.current?.focus();
  };

  return (
    <section className="settings-panel" aria-label="Settings">
      <header className="settings-header">
        <div>
          <h1>Settings</h1>
          <p className="panel-subtitle" style={{ margin: 0 }}>
            Coreside settings control the app itself. App settings are added by
            the apps you create.
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
            <div className="settings-search-shell">
              <Search
                className="settings-search-icon"
                size={18}
                aria-hidden
              />
              <input
                ref={searchInputRef}
                id={searchInputId}
                type="search"
                className="settings-search-input"
                placeholder="Search settings"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Escape" && searchActive) {
                    e.preventDefault();
                    clearSearch();
                  }
                }}
                autoComplete="off"
                aria-describedby={searchActive ? searchStatusId : undefined}
              />
              {searchActive ? (
                <button
                  type="button"
                  className="settings-search-clear"
                  aria-label="Clear search"
                  onClick={clearSearch}
                >
                  <X size={16} aria-hidden />
                </button>
              ) : null}
            </div>
            <div
              id={searchStatusId}
              className="visually-hidden"
              role="status"
              aria-live="polite"
            >
              {searchActive
                ? searchHits.length === 0
                  ? "No matching settings"
                  : `${searchHits.length} matching setting${searchHits.length === 1 ? "" : "s"}`
                : ""}
            </div>
            {searchHits.length > 0 ? (
              <ul
                id={searchResultsId}
                className="settings-search-results"
                role="region"
                aria-label="Matching settings"
              >
                {searchHits.map((hit) => (
                  <li key={hit.id}>
                    <button
                      type="button"
                      className="settings-search-hit"
                      onClick={() => selectSearchHit(hit)}
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
            {searchActive && searchHits.length === 0 ? (
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
                  data-coreside-tour={
                    cat.id === "help-learning"
                      ? "help-learning-nav"
                      : cat.id === "ai-access"
                        ? "ai-connections-nav"
                        : undefined
                  }
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

          <nav className="settings-nav-list" aria-label="App settings">
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
          aria-busy={categoryTransitioning}
        >
          <div className="settings-category-header">
            <h2 id={categoryHeadingId} className="settings-group-title">
              {active?.label ?? "Settings"}
            </h2>
            {active?.description ? (
              <p className="settings-group-desc">{active.description}</p>
            ) : null}
          </div>

          {paneMounted ? (
            <div
              className={`settings-category-body presence-${panePhase}${paneReducedMotion ? " reduced-motion" : ""}`}
              onTransitionEnd={(e) => {
                if (e.target !== e.currentTarget) return;
                if (e.propertyName !== "opacity") return;
                if (panePhase === "entering") onPaneEnterComplete();
                if (panePhase === "exiting") onPaneExitComplete();
              }}
            >
          {displayCategory === "general" ? (
            <section className="settings-section" aria-labelledby="general-heading">
              <h3 id="general-heading" className="visually-hidden">
                General
              </h3>
              <h4
                className="settings-subheading"
                id={settingsTargetDomId("adaptive-window")}
              >
                Adaptive window sizing
              </h4>
              <p>
                Coreside may smoothly expand the current window when a tool needs
                more usable room, while remaining inside the current monitor.
              </p>
              <div
                className="theme-options"
                role="group"
                aria-labelledby={settingsTargetDomId("adaptive-window")}
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

          {displayCategory === "appearance" ? (
            <div className="settings-group">
            <section
              className="settings-section"
              aria-labelledby={settingsTargetDomId("theme")}
            >
              <h3 id={settingsTargetDomId("theme")}>Theme</h3>
              <p>
                Choose how Coreside looks. Wallpapers and panel transparency are
                below.
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

              <h4 className="settings-subheading" id={settingsTargetDomId("dock-icon")}>
                Dock icon
              </h4>
              <p>
                macOS Dock tile. Auto follows system appearance; Dark and Light
                lock one of the two brand icons.
              </p>
              <div
                className="dock-icon-options"
                role="group"
                aria-labelledby={settingsTargetDomId("dock-icon")}
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
            <section
              className="settings-section"
              aria-labelledby={settingsTargetDomId("wallpaper-link")}
            >
              <h3 id={settingsTargetDomId("wallpaper-link")}>Wallpapers</h3>
              <p>
                Workspace wallpaper presets. Readability stays enforced for
                protected UI.
              </p>
              <WallpaperSettings />
            </section>
            </div>
          ) : null}

          {displayCategory === "ai-access" ? (
            <div id={settingsTargetDomId("ai-provider")}>
              <div id={settingsTargetDomId("api-key")} className="visually-hidden" tabIndex={-1}>
                API key
              </div>
              <AiProviderSettings />
            </div>
          ) : null}

          {displayCategory === "agent" ? (
            <div id={settingsTargetDomId("action-log")}>
              <AgentBehaviorSettings />
            </div>
          ) : null}

          {displayCategory === "search" ? (
            <div id={settingsTargetDomId("exa")}>
              <SearchSettingsSection />
            </div>
          ) : null}

          {displayCategory === "privacy" ? (
            <div className="settings-group">
              <section
                className="settings-section"
                aria-labelledby={settingsTargetDomId("privacy")}
              >
                <h3 id={settingsTargetDomId("privacy")}>Your data</h3>
                <p>
                  Chats, apps, and most research caches stay on this computer.
                  Provider keys use the operating system credential store when
                  available.
                </p>
                <ul className="settings-plain-list">
                  <li>
                    AI connection:{" "}
                    {aiStatus?.accessMode === "coreside_hosted"
                      ? "Coreside AI may send conversation content needed for replies."
                      : aiStatus?.accessMode === "user_byok"
                        ? "Your connected AI provider receives messages you send."
                        : aiStatus?.accessMode === "user_local"
                          ? "Local AI keeps model traffic on this machine when configured."
                          : "No AI provider is connected."}
                  </li>
                  <li>
                    Product analytics and automatic crash upload are not enabled
                    in this build. Local diagnostic export remains available.
                  </li>
                  <li>Activity stores sanitized steps only when you enable it.</li>
                </ul>
                <div className="button-row">
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => selectCategory("ai-access")}
                  >
                    Open AI connections
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
              <RuntimePermissionsSettings />
              <section className="settings-section" aria-labelledby="security-status-heading">
                <h3 id="security-status-heading">Security status</h3>
                <ul className="settings-plain-list">
                  <li>Credentials: protected by OS secure storage when available</li>
                  <li>
                    Database:{" "}
                    {storageLoading
                      ? "Checking…"
                      : storageSummary == null
                        ? "Status unavailable"
                        : storageSummary.profileReady
                          ? "Profile available"
                          : "Recovery may be required"}
                  </li>
                  <li>Update verification: follow the release channel for signed builds</li>
                </ul>
              </section>
            </div>
          ) : null}

          {displayCategory === "data" ? (
            <div className="settings-group">
              <section className="settings-section" aria-labelledby="storage-overview-heading">
                <h3 id="storage-overview-heading">Storage overview</h3>
                {storageLoading ? (
                  <p className="muted">Loading storage summary…</p>
                ) : storageSummary ? (
                  <ul className="settings-plain-list">
                    <li>{storageSummary.chatCount} chats</li>
                    <li>{storageSummary.projectCount} projects</li>
                    <li>{storageSummary.toolCount} apps</li>
                    <li>
                      Database size:{" "}
                      {storageSummary.databaseBytes != null
                        ? `${(storageSummary.databaseBytes / (1024 * 1024)).toFixed(2)} MB`
                        : "—"}
                    </li>
                    <li>
                      Media size:{" "}
                      {storageSummary.mediaBytes != null
                        ? `${(storageSummary.mediaBytes / (1024 * 1024)).toFixed(2)} MB`
                        : "—"}
                    </li>
                    <li>
                      Backups size:{" "}
                      {storageSummary.backupBytes != null
                        ? `${(storageSummary.backupBytes / (1024 * 1024)).toFixed(2)} MB`
                        : "—"}
                    </li>
                  </ul>
                ) : (
                  <p className="muted">Could not load storage summary.</p>
                )}
              </section>

              <section className="settings-section" aria-labelledby={settingsTargetDomId("backup")}>
                <h3 id={settingsTargetDomId("backup")}>Backup</h3>
                <p>
                  Create a verified local profile backup archive. Current backups
                  include a consistent database snapshot. Media and attachments
                  inclusion is expanding. Provider keys and credentials are never
                  included.
                </p>
                <div className="button-row">
                  <button
                    type="button"
                    className="btn btn-primary"
                    disabled={backupBusy}
                    aria-busy={backupBusy}
                    onClick={() => {
                      setBackupBusy(true);
                      setBackupMessage(null);
                      void api
                        .createProfileBackup()
                        .then((result) => {
                          setBackupMessage(
                            `Backup saved as ${result.path} (${(result.byteSize / 1024).toFixed(0)} KB, ${result.format}).`,
                          );
                        })
                        .catch((err: unknown) => {
                          setBackupMessage(
                            err instanceof Error
                              ? err.message
                              : "Backup failed.",
                          );
                        })
                        .finally(() => setBackupBusy(false));
                    }}
                  >
                    {backupBusy ? "Backing up…" : "Back up now"}
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => {
                      void api
                        .getDatabaseHealth()
                        .then((report) => setDbHealth(report))
                        .catch((err: unknown) => {
                          setBackupMessage(
                            err instanceof Error
                              ? err.message
                              : "Health check failed.",
                          );
                        });
                    }}
                  >
                    Check database
                  </button>
                </div>
                {backupMessage ? (
                  <p className="muted" role="status">
                    {backupMessage}
                  </p>
                ) : null}
                {dbHealth ? (
                  <p className="muted" role="status">
                    Health: {dbHealth.status} · quick_check={dbHealth.quickCheck} ·
                    foreign_keys={dbHealth.foreignKeyCheck}
                    {dbHealth.schemaVersion
                      ? ` · schema ${dbHealth.schemaVersion}`
                      : ""}
                  </p>
                ) : null}
              </section>

              <section className="settings-section" aria-labelledby="data-heading">
                <h3 id="data-heading">Delete local data</h3>
                <p>
                  Clearing data cannot be undone from this screen. Create a backup
                  first when possible.
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
                      Clear apps
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
                        Confirm clear apps
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
                <button
                  type="button"
                  className="btn btn-ghost"
                  style={{ marginTop: "0.75rem" }}
                  onClick={() => selectCategory("advanced")}
                >
                  Open Recovery
                </button>
              </section>
            </div>
          ) : null}

          {displayCategory === "accessibility" ? (
            <section
              className="settings-section"
              aria-labelledby={settingsTargetDomId("reduced-motion")}
            >
              <h3 id={settingsTargetDomId("reduced-motion")}>Accessibility</h3>
              <p>
                Coreside respects your system preference for reduced motion. When
                reduced motion is enabled, non-essential animations are minimized.
              </p>
              <p className="muted">
                Preference is read from the system — it is not overridden here.
              </p>
            </section>
          ) : null}

          {displayCategory === "advanced" ? (
            <div className="settings-group">
              <div id={settingsTargetDomId("recovery")}>
                <RecoverySettings />
              </div>
              <div id={settingsTargetDomId("runtime-permissions")}>
                <RuntimePermissionsSettings />
              </div>
            </div>
          ) : null}

          {displayCategory === "help-learning" ? (
            <div className="settings-group">
              <section
                className="settings-section"
                aria-labelledby="whats-new-heading"
              >
                <h3 id="whats-new-heading">What&apos;s new</h3>
                <p className="muted">
                  {WHATS_NEW_VERSION_LABEL} highlights for Coreside.
                </p>
                <ul className="settings-plain-list">
                  {WHATS_NEW_ITEMS.map((item) => (
                    <li key={item.title}>
                      <strong>{item.title}</strong>
                      <span className="muted"> — {item.body}</span>
                    </li>
                  ))}
                </ul>
              </section>
              <section
                className="settings-section"
                aria-labelledby={settingsTargetDomId("help-learning")}
              >
                <h3 id={settingsTargetDomId("help-learning")}>Tours</h3>
                <p>
                  Learn Coreside offline. Tours do not require an AI provider.
                </p>
                <div className="button-row">
                  <button
                    type="button"
                    className="btn btn-primary"
                    onClick={() => void startEssentials(false)}
                  >
                    Restart essentials
                  </button>
                  {unfinishedEssentials ? (
                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={() => void startEssentials(true)}
                    >
                      Continue unfinished
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="btn btn-ghost"
                    onClick={() => void resetEssentials()}
                  >
                    Reset progress
                  </button>
                </div>
                <ul className="settings-plain-list" style={{ marginTop: "1rem" }}>
                  {listConsumerTutorials(developerMode).map((mod) => (
                    <li key={mod.id}>
                      <button
                        type="button"
                        className="btn btn-secondary"
                        style={{ marginTop: 4 }}
                        onClick={() => void startModule(mod.id, true)}
                      >
                        {mod.title}
                        {mod.developerOnly ? " (developer)" : ""}
                      </button>
                      <span className="muted"> — {mod.description}</span>
                    </li>
                  ))}
                </ul>
              </section>
              <section className="settings-section">
                <h3>Related</h3>
                <div className="button-row">
                  <button
                    type="button"
                    className="btn btn-ghost"
                    onClick={() => selectCategory("privacy")}
                  >
                    Privacy & Security
                  </button>
                  <button
                    type="button"
                    className="btn btn-ghost"
                    onClick={() => selectCategory("data")}
                  >
                    Backup & data
                  </button>
                  <button
                    type="button"
                    className="btn btn-ghost"
                    onClick={() => selectCategory("about")}
                  >
                    About
                  </button>
                </div>
              </section>
            </div>
          ) : null}

          {displayCategory === "about" ? (
            <section className="settings-section" aria-labelledby={settingsTargetDomId("version")}>
              <h3 id={settingsTargetDomId("version")}>About</h3>
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
                      "An AI-native personal software environment that can grow apps beside your conversations."}
                  </p>
                </div>
              </div>
            </section>
          ) : null}

          {displayCategory === "added" ? (
            <div className="settings-group">
              <section
                className="settings-section settings-section-added"
                id={settingsTargetDomId("app-settings")}
              >
                <h3>App settings</h3>
                {addedLoading ? (
                  <p className="muted">Loading…</p>
                ) : addedSettings.length === 0 ? (
                  <div className="settings-empty">
                    <p>
                      Settings created for your personal apps will appear here.
                      Wallpapers live under Appearance.
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
                            ? ` · app ${setting.ownerToolId}`
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
          ) : null}
        </div>
      </div>
    </section>
  );
}
