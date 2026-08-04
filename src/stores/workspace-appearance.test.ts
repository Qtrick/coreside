import { beforeEach, describe, expect, it, vi } from "vitest";

const setWorkspaceAppearance = vi.fn();

vi.mock("@/lib/tauri", () => ({
  api: {
    setWorkspaceAppearance: (...args: unknown[]) =>
      setWorkspaceAppearance(...args),
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

  it("reuses one in-flight promise for same-target double commit (pointerup+blur)", async () => {
    const { useAppStore } = await import("@/stores/app-store");
    useAppStore.setState({ interfaceTransparency: 20 });

    let resolvePersist!: (value: {
      interfaceTransparency: number;
      wallpaper: { kind: string };
      wallpaperJson: null;
    }) => void;
    setWorkspaceAppearance.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolvePersist = resolve;
        }),
    );

    const first = useAppStore.getState().commitInterfaceTransparency(35);
    const second = useAppStore.getState().commitInterfaceTransparency(35);
    expect(setWorkspaceAppearance).toHaveBeenCalledTimes(1);

    resolvePersist({
      interfaceTransparency: 35,
      wallpaper: { kind: "none" },
      wallpaperJson: null,
    });
    await Promise.all([first, second]);
    expect(setWorkspaceAppearance).toHaveBeenCalledTimes(1);
    expect(useAppStore.getState().interfaceTransparency).toBe(35);
  });

  it("applyWorkspaceWallpaper previews immediately, commits on success, rolls back on failure", async () => {
    const { useAppStore } = await import("@/stores/app-store");

    // Sync module committed baseline (setState alone does not).
    setWorkspaceAppearance.mockResolvedValueOnce({
      wallpaper: { kind: "none" },
      wallpaperJson: null,
      interfaceTransparency: 20,
    });
    await useAppStore.getState().applyWorkspaceWallpaper("");
    expect(useAppStore.getState().globalWallpaperJson).toBeNull();

    const schema = JSON.stringify({
      schemaVersion: "1",
      type: "canvas-preset",
      preset: "aurora",
    });

    // Failure path: optimistic preview then restore previousCommitted.
    let rejectPersist!: (reason?: unknown) => void;
    setWorkspaceAppearance.mockImplementationOnce(
      () =>
        new Promise((_, reject) => {
          rejectPersist = reject;
        }),
    );
    const failing = useAppStore.getState().applyWorkspaceWallpaper(schema);
    expect(useAppStore.getState().globalWallpaperJson).toBe(schema);
    expect(setWorkspaceAppearance).toHaveBeenCalledWith({
      wallpaperJson: schema,
    });
    rejectPersist(new Error("apply failed"));
    await expect(failing).rejects.toThrow("apply failed");
    expect(useAppStore.getState().globalWallpaperJson).toBeNull();
    expect(useAppStore.getState().wallpaper).toEqual({ kind: "none" });

    // Success path: preview then mark committed from settings response.
    setWorkspaceAppearance.mockResolvedValueOnce({
      wallpaper: { kind: "none" },
      wallpaperJson: schema,
      interfaceTransparency: 20,
    });
    const committing = useAppStore.getState().applyWorkspaceWallpaper(schema);
    expect(useAppStore.getState().globalWallpaperJson).toBe(schema);
    await committing;
    expect(useAppStore.getState().globalWallpaperJson).toBe(schema);

    // A later failure must roll back to the last committed wallpaper, not clear.
    setWorkspaceAppearance.mockRejectedValueOnce(new Error("second fail"));
    const matrix = JSON.stringify({
      schemaVersion: "1",
      type: "canvas-preset",
      preset: "matrix",
    });
    await expect(
      useAppStore.getState().applyWorkspaceWallpaper(matrix),
    ).rejects.toThrow("second fail");
    expect(useAppStore.getState().globalWallpaperJson).toBe(schema);
  });

  it("overlapping transparency commit: stale success still anchors rollback of newer failure", async () => {
    const { useAppStore } = await import("@/stores/app-store");

    setWorkspaceAppearance.mockResolvedValueOnce({
      interfaceTransparency: 30,
      wallpaper: { kind: "none" },
      wallpaperJson: null,
    });
    await useAppStore.getState().commitInterfaceTransparency(30);
    expect(useAppStore.getState().interfaceTransparency).toBe(30);

    let resolveOlder!: (value: {
      interfaceTransparency: number;
      wallpaper: { kind: string };
      wallpaperJson: null;
    }) => void;
    let rejectNewer!: (reason?: unknown) => void;

    setWorkspaceAppearance.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveOlder = resolve;
        }),
    );
    const first = useAppStore.getState().commitInterfaceTransparency(40);
    expect(useAppStore.getState().interfaceTransparency).toBe(40);

    setWorkspaceAppearance.mockImplementationOnce(
      () =>
        new Promise((_, reject) => {
          rejectNewer = reject;
        }),
    );
    const second = useAppStore.getState().commitInterfaceTransparency(55);
    expect(useAppStore.getState().interfaceTransparency).toBe(55);

    resolveOlder({
      interfaceTransparency: 40,
      wallpaper: { kind: "none" },
      wallpaperJson: null,
    });
    await first;
    // Newer preview must remain painted while still in flight.
    expect(useAppStore.getState().interfaceTransparency).toBe(55);

    rejectNewer(new Error("newer failed"));
    await expect(second).rejects.toThrow("newer failed");
    // Roll back to 40 (DB truth from overlapping older success), not 30.
    expect(useAppStore.getState().interfaceTransparency).toBe(40);
  });

  it("overlapping wallpaper apply: stale success still anchors rollback of newer failure", async () => {
    const { useAppStore } = await import("@/stores/app-store");

    setWorkspaceAppearance.mockResolvedValueOnce({
      wallpaper: { kind: "none" },
      wallpaperJson: null,
      interfaceTransparency: 20,
    });
    await useAppStore.getState().applyWorkspaceWallpaper("");

    const aurora = JSON.stringify({
      schemaVersion: "1",
      type: "canvas-preset",
      preset: "aurora",
    });
    const matrix = JSON.stringify({
      schemaVersion: "1",
      type: "canvas-preset",
      preset: "matrix",
    });

    let resolveAurora!: (value: {
      wallpaper: { kind: string };
      wallpaperJson: string;
      interfaceTransparency: number;
    }) => void;
    let rejectMatrix!: (reason?: unknown) => void;

    setWorkspaceAppearance.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveAurora = resolve;
        }),
    );
    const first = useAppStore.getState().applyWorkspaceWallpaper(aurora);
    expect(useAppStore.getState().globalWallpaperJson).toBe(aurora);

    setWorkspaceAppearance.mockImplementationOnce(
      () =>
        new Promise((_, reject) => {
          rejectMatrix = reject;
        }),
    );
    const second = useAppStore.getState().applyWorkspaceWallpaper(matrix);
    expect(useAppStore.getState().globalWallpaperJson).toBe(matrix);

    // Older apply succeeds while newer is still in flight — must record commit
    // truth even though it must not paint over the newer preview.
    resolveAurora({
      wallpaper: { kind: "none" },
      wallpaperJson: aurora,
      interfaceTransparency: 20,
    });
    await first;
    expect(useAppStore.getState().globalWallpaperJson).toBe(matrix);

    rejectMatrix(new Error("matrix failed"));
    await expect(second).rejects.toThrow("matrix failed");
    // Roll back to aurora (DB truth from overlapping older success), not none.
    expect(useAppStore.getState().globalWallpaperJson).toBe(aurora);
  });
});
