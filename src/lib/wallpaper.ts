import type { Project } from "@/types/project";
import type { Conversation } from "@/types/messages";
import type { WallpaperConfig, WallpaperKind } from "@/types/agent";
import { WallpaperKindSchema } from "@/types/agent";
import type { AppView } from "@/lib/navigation";
import {
  parseWallpaperJson,
  type ResolvedWallpaper,
} from "@/types/wallpaper";

/**
 * Canvas preset id currently applied to the workspace.
 * Prefers `wallpaperJson` (schema) over legacy `wallpaper`; returns `null`
 * when a non-preset schema wallpaper is active (e.g. media cover).
 * This pass is Apply-only — there is no separate preview selection state.
 */
export function activeCanvasPresetId(input: {
  globalWallpaperJson?: string | null;
  globalWallpaper: WallpaperConfig;
}): WallpaperKind | null {
  const { globalWallpaperJson, globalWallpaper } = input;

  if (globalWallpaperJson?.trim()) {
    const parsed = parseWallpaperJson(globalWallpaperJson);
    if (parsed.format === "schema") {
      // Solid-color schema wallpapers are the schema equivalent of preset "None".
      if (parsed.config.type === "static-color") {
        return "none";
      }
      if (parsed.config.type === "floating-particles") {
        return "particles";
      }
      if (parsed.config.type === "canvas-preset" && parsed.config.preset) {
        const kind = WallpaperKindSchema.safeParse(parsed.config.preset);
        if (kind.success && kind.data !== "none") return kind.data;
      }
      return null;
    }
    if (parsed.format === "legacy") {
      return parsed.config.kind;
    }
    return "none";
  }

  if (globalWallpaper.kind && globalWallpaper.kind !== "none") {
    return globalWallpaper.kind;
  }
  return "none";
}

export function resolveActiveWallpaper(input: {
  view: AppView;
  globalWallpaper: WallpaperConfig;
  globalWallpaperJson?: string | null;
  conversations: Conversation[];
  projects: Project[];
  activeConversationId: string | null;
  activeProject: Project | null;
}): ResolvedWallpaper {
  const {
    view,
    globalWallpaper,
    globalWallpaperJson,
    conversations,
    projects,
    activeConversationId,
    activeProject,
  } = input;

  if (view.kind === "chat" && activeConversationId) {
    const conversation = conversations.find((c) => c.id === activeConversationId);
    if (conversation?.projectId) {
      const project = projects.find((p) => p.id === conversation.projectId);
      if (project?.wallpaperJson) {
        const parsed = parseWallpaperJson(project.wallpaperJson);
        if (parsed.format !== "none") return parsed;
      }
    }
  }

  if (view.kind === "project" && activeProject?.wallpaperJson) {
    const parsed = parseWallpaperJson(activeProject.wallpaperJson);
    if (parsed.format !== "none") return parsed;
  }

  if (globalWallpaperJson) {
    const parsed = parseWallpaperJson(globalWallpaperJson);
    if (parsed.format !== "none") return parsed;
  }

  if (globalWallpaper.kind && globalWallpaper.kind !== "none") {
    return { format: "legacy", config: globalWallpaper };
  }

  return { format: "none" };
}
