import { useState, useRef, useEffect } from "react";
import {
  Image,
  LayoutGrid,
  MessageSquare,
  Moon,
  RefreshCw,
  Settings,
  Sun,
  User,
} from "lucide-react";
import { CoresideLogo } from "@/components/branding/CoresideLogo";
import { useAppStore } from "@/stores/app-store";
import { useShallow } from "zustand/react/shallow";

export type RailDestination = "workspace" | "apps" | "media" | "automations" | "settings";

interface IconRailProps {
  activeDestination: RailDestination;
  onSelectDestination: (dest: RailDestination) => void;
}

export function IconRail({
  activeDestination,
  onSelectDestination,
}: IconRailProps) {
  const [profileMenuOpen, setProfileMenuOpen] = useState(false);
  const profileRef = useRef<HTMLDivElement>(null);
  const avatarBtnRef = useRef<HTMLButtonElement>(null);

  const {
    resolvedTheme,
    setTheme,
    navigateToSettings,
    navigateToMedia,
    navigateToAutomations,
    navigateToChat,
    activeConversationId,
  } = useAppStore(
    useShallow((s) => ({
      resolvedTheme: s.resolvedTheme,
      setTheme: s.setTheme,
      navigateToSettings: s.navigateToSettings,
      navigateToMedia: s.navigateToMedia,
      navigateToAutomations: s.navigateToAutomations,
      navigateToChat: s.navigateToChat,
      activeConversationId: s.activeConversationId,
    })),
  );

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (
        profileRef.current &&
        !profileRef.current.contains(event.target as Node)
      ) {
        setProfileMenuOpen(false);
      }
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setProfileMenuOpen(false);
        avatarBtnRef.current?.focus();
      }
    }
    if (profileMenuOpen) {
      document.addEventListener("mousedown", handleClickOutside);
      document.addEventListener("keydown", handleKeyDown);
      return () => {
        document.removeEventListener("mousedown", handleClickOutside);
        document.removeEventListener("keydown", handleKeyDown);
      };
    }
  }, [profileMenuOpen]);

  const toggleTheme = () => {
    const nextTheme = resolvedTheme === "dark" ? "light" : "dark";
    void setTheme(nextTheme);
  };

  return (
    <nav
      className="icon-rail"
      aria-label="Primary navigation rail"
      data-testid="icon-rail"
    >
      <div className="icon-rail-top">
        {/* macOS window control clearance area */}
        <div className="icon-rail-window-drag" aria-hidden="true" />

        {/* Coreside brand logo / home button */}
        <button
          type="button"
          className="icon-rail-logo-btn"
          onClick={() => {
            if (activeConversationId) {
              void navigateToChat(activeConversationId);
            } else {
              useAppStore.setState({ view: { kind: "chat", conversationId: null } });
            }
            onSelectDestination("workspace");
          }}
          aria-label="Coreside home"
          title="Coreside Home"
        >
          <CoresideLogo appearance={resolvedTheme} size={28} />
        </button>
      </div>

      <div className="icon-rail-nav">
        {/* 1. Workspace (Chats & Projects) */}
        <button
          type="button"
          className={`icon-rail-btn${activeDestination === "workspace" ? " active" : ""}`}
          onClick={() => onSelectDestination("workspace")}
          aria-label="Chats and Projects"
          title="Chats & Projects"
          aria-current={activeDestination === "workspace" ? "page" : undefined}
        >
          <MessageSquare size={20} aria-hidden />
        </button>

        {/* 2. Apps */}
        <button
          type="button"
          className={`icon-rail-btn${activeDestination === "apps" ? " active" : ""}`}
          onClick={() => onSelectDestination("apps")}
          aria-label="Personal Apps"
          title="Apps"
          aria-current={activeDestination === "apps" ? "page" : undefined}
        >
          <LayoutGrid size={20} aria-hidden />
        </button>

        {/* 3. Media */}
        <button
          type="button"
          className={`icon-rail-btn${activeDestination === "media" ? " active" : ""}`}
          onClick={() => {
            onSelectDestination("media");
            navigateToMedia();
          }}
          aria-label="Media library"
          title="Media Library"
          aria-current={activeDestination === "media" ? "page" : undefined}
        >
          <Image size={20} aria-hidden />
        </button>

        {/* 4. Automations */}
        <button
          type="button"
          className={`icon-rail-btn${activeDestination === "automations" ? " active" : ""}`}
          onClick={() => {
            onSelectDestination("automations");
            navigateToAutomations();
          }}
          aria-label="Automations"
          title="Automations"
          aria-current={activeDestination === "automations" ? "page" : undefined}
        >
          <RefreshCw size={20} aria-hidden />
        </button>

        {/* 5. Settings */}
        <button
          type="button"
          className={`icon-rail-btn${activeDestination === "settings" ? " active" : ""}`}
          onClick={() => {
            onSelectDestination("settings");
            navigateToSettings();
          }}
          aria-label="Settings"
          title="Settings"
          aria-current={activeDestination === "settings" ? "page" : undefined}
        >
          <Settings size={20} aria-hidden />
        </button>
      </div>

      {/* Bottom Profile / Avatar control (Sole profile control in window) */}
      <div className="icon-rail-bottom" ref={profileRef}>
        <button
          ref={avatarBtnRef}
          type="button"
          className="icon-rail-avatar-btn"
          onClick={() => setProfileMenuOpen((prev) => !prev)}
          aria-label="Personal profile and quick settings"
          title="Personal Profile"
          aria-expanded={profileMenuOpen}
          aria-haspopup="dialog"
        >
          <div className="avatar-circle">
            <User size={16} aria-hidden />
          </div>
        </button>

        {profileMenuOpen ? (
          <div
            className="icon-rail-profile-popover"
            role="dialog"
            aria-label="Profile menu"
          >
            <div className="profile-header">
              <div className="profile-name">Local Profile</div>
              <div className="profile-badge">Offline Local-First</div>
            </div>
            <div className="profile-divider" />
            <button
              type="button"
              className="profile-menu-item"
              onClick={() => {
                toggleTheme();
                setProfileMenuOpen(false);
              }}
            >
              {resolvedTheme === "dark" ? <Sun size={15} /> : <Moon size={15} />}
              <span>{resolvedTheme === "dark" ? "Light theme" : "Dark theme"}</span>
            </button>
            <button
              type="button"
              className="profile-menu-item"
              onClick={() => {
                onSelectDestination("settings");
                navigateToSettings();
                setProfileMenuOpen(false);
              }}
            >
              <Settings size={15} />
              <span>All Settings</span>
            </button>
          </div>
        ) : null}
      </div>
    </nav>
  );
}

