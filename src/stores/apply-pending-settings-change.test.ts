import { beforeEach, describe, expect, it, vi } from "vitest";
import { AppSettingsSchema } from "@/types/agent";
import type { ChatMessage } from "@/types/messages";

const applyAppearanceBundle = vi.fn();

vi.mock("@/lib/tauri", () => ({
  api: {
    applyAppearanceBundle: (...args: unknown[]) => applyAppearanceBundle(...args),
    setWorkspaceAppearance: vi.fn(),
    subscribeConversationSync: vi.fn(async () => () => undefined),
    getSurface: vi.fn(async () => null),
  },
  TauriCommandError: class TauriCommandError extends Error {
    code: string;
    constructor(message: string, code = "error") {
      super(message);
      this.code = code;
    }
  },
  listenAgentTurn: vi.fn(),
}));

describe("applyPendingSettingsChange", () => {
  beforeEach(() => {
    applyAppearanceBundle.mockReset();
  });

  it("commits pending appearance through applyAppearanceBundle atomically", async () => {
    applyAppearanceBundle.mockResolvedValueOnce({
      theme: "dark",
      wallpaper: { kind: "none" },
      wallpaperJson: "",
      interfaceTransparency: 25,
    });

    const message: ChatMessage = {
      id: "msg-settings-1",
      conversationId: "conv-1",
      role: "assistant",
      content: "Updated your theme.",
      createdAt: "2026-01-01T00:00:00.000Z",
      metadata: {
        settingsChange: { theme: "dark" },
        settingsChangeStatus: "pending",
        pending: true,
      },
    };

    const { useAppStore } = await import("@/stores/app-store");
    useAppStore.setState({
      theme: "system",
      interfaceTransparency: 25,
      globalWallpaperJson: null,
      wallpaper: { kind: "none" },
      messages: [message],
      pendingSettingsChange: {
        conversationId: "conv-1",
        messageId: "msg-settings-1",
        settingsChange: { theme: "dark" },
      },
    });

    await useAppStore.getState().applyPendingSettingsChange();

    expect(applyAppearanceBundle).toHaveBeenCalledTimes(1);
    expect(applyAppearanceBundle).toHaveBeenCalledWith({ theme: "dark" });
    expect(useAppStore.getState().pendingSettingsChange).toBeNull();
    expect(useAppStore.getState().theme).toBe("dark");
    expect(useAppStore.getState().messages[0]?.metadata?.settingsChangeStatus).toBe(
      "applied",
    );
  });

  it("does not persist live wallpaper or transparency previews with a color-only proposal", async () => {
    const committedWallpaper = { kind: "none" as const };
    applyAppearanceBundle.mockResolvedValueOnce({
      theme: "system",
      wallpaper: committedWallpaper,
      wallpaperJson: "",
      interfaceTransparency: 20,
      accentPrimaryLight: "#123456",
      accentPrimaryDark: "#abcdef",
    });

    const message: ChatMessage = {
      id: "msg-settings-preview",
      conversationId: "conv-1",
      role: "assistant",
      content: "Updated your accent color.",
      createdAt: "2026-01-01T00:00:00.000Z",
      metadata: {
        settingsChange: {
          accentPrimary: { light: "#123456", dark: "#abcdef" },
        },
        settingsChangeStatus: "pending",
        pending: true,
      },
    };

    const { useAppStore } = await import("@/stores/app-store");
    useAppStore.getState().reconcileCommittedAppearance(AppSettingsSchema.parse({
      theme: "system",
      wallpaper: committedWallpaper,
      wallpaperJson: "",
      interfaceTransparency: 20,
    }));
    useAppStore.setState({
      // These values represent an uncommitted settings-panel preview.
      interfaceTransparency: 77,
      wallpaper: { kind: "matrix", opacity: 0.8 },
      globalWallpaperJson:
        '{"schemaVersion":"1","type":"static-color","color":"#ff00ff"}',
      messages: [message],
      pendingSettingsChange: {
        conversationId: "conv-1",
        messageId: "msg-settings-preview",
        settingsChange: {
          accentPrimary: { light: "#123456", dark: "#abcdef" },
        },
      },
    });

    await useAppStore.getState().applyPendingSettingsChange();

    expect(applyAppearanceBundle).toHaveBeenCalledWith({
      accentPrimary: { light: "#123456", dark: "#abcdef" },
    });
    const changes = applyAppearanceBundle.mock.calls[0]![0] as Record<
      string,
      unknown
    >;
    expect(changes).not.toHaveProperty("wallpaper");
    expect(changes).not.toHaveProperty("wallpaperJson");
    expect(changes).not.toHaveProperty("interfaceTransparency");
  });

  it("does not mutate appearance or clear pending when applyAppearanceBundle fails", async () => {
    applyAppearanceBundle.mockRejectedValueOnce(new Error("apply failed"));

    const message: ChatMessage = {
      id: "msg-settings-2",
      conversationId: "conv-1",
      role: "assistant",
      content: "Updated your theme.",
      createdAt: "2026-01-01T00:00:00.000Z",
      metadata: {
        settingsChange: { theme: "dark" },
        settingsChangeStatus: "pending",
        pending: true,
      },
    };

    const { useAppStore } = await import("@/stores/app-store");
    useAppStore.setState({
      theme: "system",
      interfaceTransparency: 25,
      globalWallpaperJson: null,
      wallpaper: { kind: "none" },
      messages: [message],
      pendingSettingsChange: {
        conversationId: "conv-1",
        messageId: "msg-settings-2",
        settingsChange: { theme: "dark" },
      },
    });

    await useAppStore.getState().applyPendingSettingsChange();

    expect(useAppStore.getState().theme).toBe("system");
    expect(useAppStore.getState().pendingSettingsChange).not.toBeNull();
    expect(useAppStore.getState().sendError).toBe("apply failed");
    expect(useAppStore.getState().messages[0]?.metadata?.settingsChangeStatus).toBe(
      "pending",
    );
  });
});
