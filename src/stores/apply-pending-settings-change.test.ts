import { beforeEach, describe, expect, it, vi } from "vitest";
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
    const snapshot = applyAppearanceBundle.mock.calls[0]![0] as {
      themePreference: string;
      interfaceTransparency: number;
    };
    expect(snapshot.themePreference).toBe("dark");
    expect(snapshot.interfaceTransparency).toBe(25);
    expect(useAppStore.getState().pendingSettingsChange).toBeNull();
    expect(useAppStore.getState().theme).toBe("dark");
    expect(useAppStore.getState().messages[0]?.metadata?.settingsChangeStatus).toBe(
      "applied",
    );
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
