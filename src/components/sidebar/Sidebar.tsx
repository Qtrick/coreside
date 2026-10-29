import { useEffect, useState } from "react";
import { IconRail, type RailDestination } from "./IconRail";
import { ContextualSidebar, type ContextualMode } from "./ContextualSidebar";
import { NARROW_SIDEBAR_MEDIA_QUERY } from "@/lib/layout-mode";
import { useAppStore } from "@/stores/app-store";
import { useShallow } from "zustand/react/shallow";

export function Sidebar() {
  const {
    sidebarCollapsed,
    toggleSidebar,
    view,
  } = useAppStore(
    useShallow((s) => ({
      sidebarCollapsed: s.sidebarCollapsed,
      toggleSidebar: s.toggleSidebar,
      view: s.view,
    })),
  );

  const [activeRailDestination, setActiveRailDestination] = useState<RailDestination>(() => {
    if (view.kind === "settings") return "settings";
    if (view.kind === "automations") return "automations";
    if (view.kind === "media") return "media";
    if (view.kind === "tools") return "apps";
    return "workspace";
  });

  const [contextualMode, setContextualMode] = useState<ContextualMode>(() => {
    return view.kind === "tools" ? "apps" : "chats_projects";
  });

  const [narrowViewport, setNarrowViewport] = useState(
    () =>
      typeof window !== "undefined" &&
      Boolean(window.matchMedia?.(NARROW_SIDEBAR_MEDIA_QUERY).matches),
  );

  useEffect(() => {
    if (!window.matchMedia) return;
    const media = window.matchMedia(NARROW_SIDEBAR_MEDIA_QUERY);
    const update = () => setNarrowViewport(media.matches);
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);

  // Sync active destination with global view changes
  useEffect(() => {
    if (view.kind === "settings") {
      setActiveRailDestination("settings");
    } else if (view.kind === "automations") {
      setActiveRailDestination("automations");
    } else if (view.kind === "media") {
      setActiveRailDestination("media");
    } else if (view.kind === "tools") {
      setActiveRailDestination("apps");
      setContextualMode("apps");
    } else if (view.kind === "chat" || view.kind === "project" || view.kind === "projects") {
      setActiveRailDestination("workspace");
    }
  }, [view.kind]);

  const effectiveCollapsed = sidebarCollapsed || narrowViewport;

  const handleSelectRailDestination = (dest: RailDestination) => {
    setActiveRailDestination(dest);
    if (dest === "workspace") {
      setContextualMode("chats_projects");
      if (effectiveCollapsed && !narrowViewport) {
        void toggleSidebar();
      }
    } else if (dest === "apps") {
      setContextualMode("apps");
      if (effectiveCollapsed && !narrowViewport) {
        void toggleSidebar();
      }
    }
  };

  const handleContextualModeChange = (mode: ContextualMode) => {
    setContextualMode(mode);
    if (mode === "apps") {
      setActiveRailDestination("apps");
    } else {
      setActiveRailDestination("workspace");
    }
  };

  const handleCollapseContextual = () => {
    if (!narrowViewport) {
      void toggleSidebar();
    }
  };

  return (
    <aside
      className={`sidebar dual-sidebar${effectiveCollapsed ? " collapsed" : ""}`}
      aria-label="Coreside navigation"
      data-testid="dual-sidebar"
    >
      {/* 1. Permanent Left Icon Rail */}
      <IconRail
        activeDestination={activeRailDestination}
        onSelectDestination={handleSelectRailDestination}
      />

      {/* 2. Wider Contextual Sidebar (Collapsible) */}
      {!effectiveCollapsed ? (
        <ContextualSidebar
          mode={contextualMode}
          onModeChange={handleContextualModeChange}
          onCollapse={handleCollapseContextual}
        />
      ) : null}
    </aside>
  );
}
