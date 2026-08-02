import { describe, expect, it } from "vitest";
import { DEFAULT_WALLPAPER } from "@/types/agent";
import { activeCanvasPresetId, resolveActiveWallpaper } from "@/lib/wallpaper";
import type { Project } from "@/types/project";
import type { Conversation } from "@/types/messages";
import { buildCanvasPresetProposal, schemaWallpaperToJson } from "@/types/wallpaper";

const baseProject: Project = {
  id: "proj-1",
  name: "Demo",
  description: null,
  iconKey: "folder",
  instructions: null,
  summary: null,
  summaryUpdatedAt: null,
  archived: false,
  pinned: false,
  wallpaperJson: JSON.stringify({
    schemaVersion: "1",
    type: "image-cover",
    assetId: "asset-1",
  }),
  createdAt: "2026-01-01T00:00:00.000Z",
  updatedAt: "2026-01-01T00:00:00.000Z",
  lastOpenedAt: null,
};

const chatInProject: Conversation = {
  id: "chat-1",
  title: "Chat",
  workspaceId: "ws-personal-default",
  projectId: "proj-1",
  pinned: false,
  archived: false,
  createdAt: "2026-01-01T00:00:00.000Z",
  updatedAt: "2026-01-01T00:00:00.000Z",
};

describe("resolveActiveWallpaper", () => {
  it("prefers project wallpaper for project chats", () => {
    const resolved = resolveActiveWallpaper({
      view: { kind: "chat", conversationId: "chat-1" },
      globalWallpaper: { kind: "matrix" },
      globalWallpaperJson: null,
      conversations: [chatInProject],
      projects: [baseProject],
      activeConversationId: "chat-1",
      activeProject: null,
    });
    expect(resolved.format).toBe("schema");
    if (resolved.format === "schema") {
      expect(resolved.config.assetId).toBe("asset-1");
    }
  });

  it("falls back to global schema wallpaper", () => {
    const resolved = resolveActiveWallpaper({
      view: { kind: "chat", conversationId: "chat-2" },
      globalWallpaper: DEFAULT_WALLPAPER,
      globalWallpaperJson: JSON.stringify({
        schemaVersion: "1",
        type: "static-color",
        color: "#101010",
      }),
      conversations: [
        {
          ...chatInProject,
          id: "chat-2",
          projectId: null,
        },
      ],
      projects: [baseProject],
      activeConversationId: "chat-2",
      activeProject: null,
    });
    expect(resolved.format).toBe("schema");
    if (resolved.format === "schema") {
      expect(resolved.config.type).toBe("static-color");
    }
  });

  it("uses legacy global wallpaper when no schema is set", () => {
    const resolved = resolveActiveWallpaper({
      view: { kind: "chat", conversationId: "chat-2" },
      globalWallpaper: { kind: "aurora" },
      globalWallpaperJson: null,
      conversations: [
        {
          ...chatInProject,
          id: "chat-2",
          projectId: null,
        },
      ],
      projects: [],
      activeConversationId: "chat-2",
      activeProject: null,
    });
    expect(resolved.format).toBe("legacy");
    if (resolved.format === "legacy") {
      expect(resolved.config.kind).toBe("aurora");
    }
  });
});

describe("activeCanvasPresetId", () => {
  it("reads canvas-preset from wallpaperJson even when legacy is none", () => {
    const proposal = buildCanvasPresetProposal("matrix");
    expect("type" in proposal).toBe(true);
    const json =
      "type" in proposal
        ? schemaWallpaperToJson(proposal)
        : JSON.stringify(proposal);
    expect(
      activeCanvasPresetId({
        globalWallpaperJson: json,
        globalWallpaper: DEFAULT_WALLPAPER,
      }),
    ).toBe("matrix");
  });

  it("maps floating-particles to particles", () => {
    const proposal = buildCanvasPresetProposal("particles");
    expect("type" in proposal).toBe(true);
    const json =
      "type" in proposal
        ? schemaWallpaperToJson(proposal)
        : JSON.stringify(proposal);
    expect(
      activeCanvasPresetId({
        globalWallpaperJson: json,
        globalWallpaper: DEFAULT_WALLPAPER,
      }),
    ).toBe("particles");
  });

  it("falls back to legacy wallpaper when wallpaperJson is empty", () => {
    expect(
      activeCanvasPresetId({
        globalWallpaperJson: null,
        globalWallpaper: { kind: "rain" },
      }),
    ).toBe("rain");
  });

  it("returns null for non-preset schema wallpapers", () => {
    expect(
      activeCanvasPresetId({
        globalWallpaperJson: JSON.stringify({
          schemaVersion: "1",
          type: "image-cover",
          assetId: "asset-1",
        }),
        globalWallpaper: DEFAULT_WALLPAPER,
      }),
    ).toBeNull();
  });

  it("returns none when both stores are clear", () => {
    expect(
      activeCanvasPresetId({
        globalWallpaperJson: null,
        globalWallpaper: DEFAULT_WALLPAPER,
      }),
    ).toBe("none");
  });

  it("treats legacy kind none JSON as none without requiring schemaVersion", () => {
    expect(
      activeCanvasPresetId({
        globalWallpaperJson: JSON.stringify({ kind: "none" }),
        globalWallpaper: { kind: "particles" },
      }),
    ).toBe("none");
  });

  it("treats static-color schema wallpaper as none", () => {
    expect(
      activeCanvasPresetId({
        globalWallpaperJson: JSON.stringify({
          schemaVersion: "1",
          type: "static-color",
          color: "#141714",
        }),
        globalWallpaper: DEFAULT_WALLPAPER,
      }),
    ).toBe("none");
  });
});
