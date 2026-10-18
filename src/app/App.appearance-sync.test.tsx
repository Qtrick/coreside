import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AppSettingsSchema } from "@/types/agent";

const getSettings = vi.fn();
let appearanceChangedHandler: (() => void) | undefined;

vi.mock("@/lib/tauri", () => ({
  api: {
    getSettings: (...args: unknown[]) => getSettings(...args),
  },
  listenAppearanceChanged: vi.fn(async (handler: () => void) => {
    appearanceChangedHandler = handler;
    return () => {
      appearanceChangedHandler = undefined;
    };
  }),
  isWebPreview: () => false,
}));

vi.mock("@/components/layout/AppShell", () => ({
  AppShell: () => <div data-testid="app-shell" />,
}));

vi.mock("@/components/recovery/BootstrapRecoveryScreen", () => ({
  BootstrapRecoveryScreen: () => null,
}));

describe("App appearance-changed listener", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    appearanceChangedHandler = undefined;
    window.location.hash = "";
    Object.defineProperty(window, "matchMedia", {
      writable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: false,
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    });
  });

  it("ignores stale getSettings responses when appearance-changed fires again", async () => {
    let resolveSlow!: (settings: ReturnType<typeof AppSettingsSchema.parse>) => void;
    let resolveFast!: (settings: ReturnType<typeof AppSettingsSchema.parse>) => void;

    const slowSettings = AppSettingsSchema.parse({
      theme: "light",
      wallpaper: { kind: "none" },
      wallpaperJson: "",
      interfaceTransparency: 20,
    });
    const fastSettings = AppSettingsSchema.parse({
      theme: "dark",
      wallpaper: { kind: "none" },
      wallpaperJson: "",
      interfaceTransparency: 20,
    });

    getSettings
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveSlow = resolve;
        }),
      )
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveFast = resolve;
        }),
      );

    const { useAppStore } = await import("@/stores/app-store");
    useAppStore.setState({
      bootstrapped: true,
      bootError: null,
      bootstrapStatus: { status: "ready" },
      theme: "system",
    });
    useAppStore.getState().applyPersistedDockIcon = vi.fn().mockResolvedValue(undefined);

    const { App } = await import("@/app/App");
    render(<App />);

    await waitFor(() => expect(appearanceChangedHandler).toBeDefined());

    act(() => {
      appearanceChangedHandler!();
      appearanceChangedHandler!();
    });

    await act(async () => {
      resolveFast(fastSettings);
    });
    await act(async () => {
      resolveSlow(slowSettings);
    });

    expect(getSettings).toHaveBeenCalledTimes(2);
    expect(useAppStore.getState().theme).toBe("dark");
  });
});
