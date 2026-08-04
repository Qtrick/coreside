import { beforeEach, describe, expect, it, vi } from "vitest";

const setWorkspaceAppearance = vi.fn();

vi.mock("@/lib/tauri", () => ({
  api: {
    setWorkspaceAppearance: (...args: unknown[]) =>
      setWorkspaceAppearance(...args),
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

describe("workspace appearance store helpers", () => {
  beforeEach(() => {
    setWorkspaceAppearance.mockReset();
    // Re-import store after mocks; reset live transparency via preview/commit.
  });

  it("preview does not invoke IPC; commit does once and rolls back on failure", async () => {
    const { useAppStore } = await import("@/stores/app-store");

    // Module committed baseline defaults to 20; same-value commit is a no-op.
    useAppStore.setState({ interfaceTransparency: 20 });
    await useAppStore.getState().commitInterfaceTransparency(20);
    expect(setWorkspaceAppearance).not.toHaveBeenCalled();

    useAppStore.getState().previewInterfaceTransparency(40);
    expect(useAppStore.getState().interfaceTransparency).toBe(40);
    expect(setWorkspaceAppearance).not.toHaveBeenCalled();

    useAppStore.getState().previewInterfaceTransparency(55);
    expect(useAppStore.getState().interfaceTransparency).toBe(55);
    expect(setWorkspaceAppearance).not.toHaveBeenCalled();

    setWorkspaceAppearance.mockRejectedValueOnce(new Error("persist failed"));
    await expect(
      useAppStore.getState().commitInterfaceTransparency(55),
    ).rejects.toThrow("persist failed");
    expect(setWorkspaceAppearance).toHaveBeenCalledTimes(1);
    expect(setWorkspaceAppearance).toHaveBeenCalledWith({
      interfaceTransparency: 55,
    });
    expect(useAppStore.getState().interfaceTransparency).toBe(20);

    setWorkspaceAppearance.mockResolvedValueOnce({
      interfaceTransparency: 40,
      wallpaper: { kind: "none" },
      wallpaperJson: null,
    });
    await useAppStore.getState().commitInterfaceTransparency(40);
    expect(useAppStore.getState().interfaceTransparency).toBe(40);

    // Duplicate commit of the committed value must not re-invoke IPC.
    setWorkspaceAppearance.mockClear();
    await useAppStore.getState().commitInterfaceTransparency(40);
    expect(setWorkspaceAppearance).not.toHaveBeenCalled();
  });

  it("applyWorkspaceWallpaper uses a single atomic invoke and updates only after success", async () => {
    const { useAppStore } = await import("@/stores/app-store");
    useAppStore.setState({
      wallpaper: { kind: "none" },
      globalWallpaperJson: null,
    });

    const schema = JSON.stringify({
      schemaVersion: "1",
      type: "canvas-preset",
      preset: "aurora",
    });

    setWorkspaceAppearance.mockRejectedValueOnce(new Error("apply failed"));
    await expect(
      useAppStore.getState().applyWorkspaceWallpaper(schema),
    ).rejects.toThrow("apply failed");
    expect(useAppStore.getState().globalWallpaperJson).toBeNull();
    expect(setWorkspaceAppearance).toHaveBeenCalledWith({
      wallpaperJson: schema,
    });

    setWorkspaceAppearance.mockResolvedValueOnce({
      wallpaper: { kind: "none" },
      wallpaperJson: schema,
      interfaceTransparency: 20,
    });
    await useAppStore.getState().applyWorkspaceWallpaper(schema);
    expect(useAppStore.getState().globalWallpaperJson).toBe(schema);
  });
});
