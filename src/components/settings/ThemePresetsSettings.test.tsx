import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ThemePresetsSettings } from "./ThemePresetsSettings";
import { api, listenThemePresetsChanged } from "@/lib/tauri";
import { DEFAULT_WALLPAPER } from "@/types/agent";
import type { AppearanceSnapshot, ThemePresetRecord } from "@/types/theme-preset";

const applyThemePreset = vi.fn();

vi.mock("@/stores/app-store", () => ({
  useAppStore: (selector: (state: {
    applyThemePreset: typeof applyThemePreset;
    activeProject: null;
  }) => unknown) =>
    selector({ applyThemePreset, activeProject: null }),
}));

vi.mock("@/lib/tauri", () => ({
  api: {
    listThemePresets: vi.fn(),
    getThemePresetCapabilities: vi.fn(),
    getAppearanceFingerprint: vi.fn(),
    saveCurrentThemePreset: vi.fn(),
    renameThemePreset: vi.fn(),
    deleteThemePreset: vi.fn(),
    updateThemePresetFromCurrent: vi.fn(),
  },
  listenThemePresetsChanged: vi.fn(async () => () => undefined),
}));

function baseSnapshot(): AppearanceSnapshot {
  return {
    schemaVersion: 1,
    themePreference: "system",
    accentPrimaryLight: "#111111",
    accentPrimaryDark: "#222222",
    accentSecondaryLight: "#333333",
    accentSecondaryDark: "#444444",
    backgroundLight: "#ffffff",
    backgroundDark: "#000000",
    surfaceLight: "#fafafa",
    surfaceDark: "#111111",
    surfaceMutedLight: "#eeeeee",
    surfaceMutedDark: "#222222",
    borderLight: "#dddddd",
    borderDark: "#333333",
    textPrimaryLight: "#000000",
    textPrimaryDark: "#ffffff",
    textSecondaryLight: "#666666",
    textSecondaryDark: "#aaaaaa",
    wallpaper: DEFAULT_WALLPAPER,
    wallpaperJson: "",
    interfaceTransparency: 20,
  };
}

function mockPreset(overrides: Partial<ThemePresetRecord> = {}): ThemePresetRecord {
  return {
    id: "preset-ocean",
    name: "Ocean",
    description: "",
    schemaVersion: 1,
    revision: 3,
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-02T00:00:00.000Z",
    snapshot: baseSnapshot(),
    ...overrides,
  };
}

describe("ThemePresetsSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.defineProperty(window, "matchMedia", {
      writable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: false,
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        addListener: vi.fn(),
        removeListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    });
    applyThemePreset.mockResolvedValue(undefined);
    vi.mocked(api.listThemePresets).mockResolvedValue([mockPreset()]);
    vi.mocked(api.getAppearanceFingerprint).mockResolvedValue("fp-test-ocean");
    vi.mocked(api.getThemePresetCapabilities).mockResolvedValue({
      used: 1,
      limit: 10,
      canCreate: true,
      planId: "free",
      planSource: "contract_default",
      freeLimit: 10,
    });
  });

  async function waitForPresetRow() {
    await waitFor(() => {
      expect(screen.getByText("Ocean")).toBeInTheDocument();
    });
  }

  async function openApplyDialog() {
    fireEvent.click(screen.getByRole("button", { name: "Apply Theme" }));
    await waitFor(() => {
      expect(
        screen.getByRole("alertdialog", { name: /Apply “Ocean”\?/ }),
      ).toBeInTheDocument();
    });
    expect(api.getAppearanceFingerprint).toHaveBeenCalled();
  }

  it("focuses Cancel when the destructive apply dialog opens", async () => {
    render(<ThemePresetsSettings />);
    await waitForPresetRow();
    await openApplyDialog();
    const dialog = screen.getByRole("alertdialog", { name: /Apply “Ocean”\?/ });

    await waitFor(() => {
      expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    });
    expect(applyThemePreset).not.toHaveBeenCalled();
  });

  it("Cancel closes the apply dialog without calling applyThemePreset", async () => {
    render(<ThemePresetsSettings />);
    await waitForPresetRow();
    await openApplyDialog();
    const dialog = screen.getByRole("alertdialog", { name: /Apply “Ocean”\?/ });
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));

    await waitFor(() => {
      expect(
        screen.queryByRole("alertdialog", { name: /Apply “Ocean”\?/ }),
      ).not.toBeInTheDocument();
    });
    expect(applyThemePreset).not.toHaveBeenCalled();
  });

  it("Escape closes the apply dialog without calling applyThemePreset", async () => {
    render(<ThemePresetsSettings />);
    await waitForPresetRow();
    await openApplyDialog();
    fireEvent.keyDown(document, { key: "Escape", code: "Escape" });

    await waitFor(() => {
      expect(
        screen.queryByRole("alertdialog", { name: /Apply “Ocean”\?/ }),
      ).not.toBeInTheDocument();
    });
    expect(applyThemePreset).not.toHaveBeenCalled();
  });

  it("does not open apply dialog when appearance fingerprint fetch fails", async () => {
    vi.mocked(api.getAppearanceFingerprint).mockRejectedValueOnce(
      new Error("Could not read appearance"),
    );
    render(<ThemePresetsSettings />);
    await waitForPresetRow();
    fireEvent.click(screen.getByRole("button", { name: "Apply Theme" }));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent("Could not read appearance");
    });
    expect(
      screen.queryByRole("alertdialog", { name: /Apply “Ocean”\?/ }),
    ).not.toBeInTheDocument();
    expect(applyThemePreset).not.toHaveBeenCalled();
  });

  it("confirm Apply calls applyThemePreset with id, revision, and fingerprint", async () => {
    render(<ThemePresetsSettings />);
    await waitForPresetRow();
    await openApplyDialog();
    const dialog = screen.getByRole("alertdialog", { name: /Apply “Ocean”\?/ });
    fireEvent.click(within(dialog).getByRole("button", { name: "Apply Theme" }));

    await waitFor(() => {
      expect(applyThemePreset).toHaveBeenCalledTimes(1);
    });
    expect(applyThemePreset).toHaveBeenCalledWith(
      "preset-ocean",
      3,
      "fp-test-ocean",
    );
  });

  it("keeps a failed apply open and allows retry with the same revision", async () => {
    applyThemePreset
      .mockRejectedValueOnce(new Error("Theme preset was updated elsewhere"))
      .mockResolvedValueOnce(undefined);
    render(<ThemePresetsSettings />);
    await waitForPresetRow();
    await openApplyDialog();
    const dialog = screen.getByRole("alertdialog", { name: /Apply “Ocean”\?/ });
    fireEvent.click(within(dialog).getByRole("button", { name: "Apply Theme" }));

    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "Theme preset was updated elsewhere",
    );
    expect(dialog).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Apply Theme" }));

    await waitFor(() => expect(dialog).not.toBeInTheDocument());
    expect(applyThemePreset).toHaveBeenCalledTimes(2);
    expect(applyThemePreset).toHaveBeenNthCalledWith(
      1,
      "preset-ocean",
      3,
      "fp-test-ocean",
    );
    expect(applyThemePreset).toHaveBeenNthCalledWith(
      2,
      "preset-ocean",
      3,
      "fp-test-ocean",
    );
  });

  it("blocks Escape and duplicate confirmation while apply is pending", async () => {
    let resolveApply!: () => void;
    applyThemePreset.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveApply = resolve;
      }),
    );
    render(<ThemePresetsSettings />);
    await waitForPresetRow();
    await openApplyDialog();
    const dialog = screen.getByRole("alertdialog", { name: /Apply “Ocean”\?/ });
    const confirm = within(dialog).getByRole("button", { name: "Apply Theme" });
    fireEvent.click(confirm);

    await waitFor(() => expect(confirm).toBeDisabled());
    fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
    fireEvent.click(confirm);
    expect(dialog).toBeInTheDocument();
    expect(applyThemePreset).toHaveBeenCalledTimes(1);

    await act(async () => resolveApply());
    await waitFor(() => expect(dialog).not.toBeInTheDocument());
  });

  it("disables Save Current Theme when quota is at limit", async () => {
    vi.mocked(api.getThemePresetCapabilities).mockResolvedValue({
      used: 10,
      limit: 10,
      canCreate: false,
      planId: "free",
      planSource: "contract_default",
      freeLimit: 10,
    });

    render(<ThemePresetsSettings />);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Save Current Theme" })).toBeDisabled();
    });
  });

  it("renders saved themes without waiting for capabilities", async () => {
    vi.mocked(api.getThemePresetCapabilities).mockReturnValue(
      new Promise(() => undefined),
    );

    render(<ThemePresetsSettings />);

    await waitForPresetRow();
    expect(screen.queryByText("Loading saved themes…")).not.toBeInTheDocument();
  });

  it("unlistens when registration resolves after unmount", async () => {
    let resolveListener!: (unlisten: () => void) => void;
    vi.mocked(listenThemePresetsChanged).mockReturnValueOnce(
      new Promise((resolve) => {
        resolveListener = resolve;
      }),
    );
    const unlisten = vi.fn();
    const view = render(<ThemePresetsSettings />);

    view.unmount();
    await act(async () => resolveListener(unlisten));

    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("does not replace a newer event refresh with an older list response", async () => {
    let resolveInitial!: (presets: ThemePresetRecord[]) => void;
    let onPresetsChanged!: () => void;
    vi.mocked(api.listThemePresets)
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveInitial = resolve;
        }),
      )
      .mockResolvedValueOnce([mockPreset({ id: "preset-forest", name: "Forest" })]);
    vi.mocked(listenThemePresetsChanged).mockImplementationOnce(async (listener) => {
      onPresetsChanged = listener;
      return () => undefined;
    });

    render(<ThemePresetsSettings />);
    await waitFor(() => expect(listenThemePresetsChanged).toHaveBeenCalledOnce());

    act(() => onPresetsChanged());
    expect(await screen.findByText("Forest")).toBeInTheDocument();

    await act(async () => {
      resolveInitial([mockPreset({ id: "preset-ocean", name: "Ocean" })]);
    });

    expect(screen.getByText("Forest")).toBeInTheDocument();
    expect(screen.queryByText("Ocean")).not.toBeInTheDocument();
  });

  it("does not replace newer capabilities with an older response", async () => {
    let resolveInitial!: (
      capabilities: Awaited<ReturnType<typeof api.getThemePresetCapabilities>>,
    ) => void;
    let onPresetsChanged!: () => void;
    vi.mocked(api.getThemePresetCapabilities)
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveInitial = resolve;
        }),
      )
      .mockResolvedValueOnce({
        used: 1,
        limit: 50,
        canCreate: true,
        planId: "pro",
        planSource: "server",
        freeLimit: 10,
      });
    vi.mocked(listenThemePresetsChanged).mockImplementationOnce(async (listener) => {
      onPresetsChanged = listener;
      return () => undefined;
    });

    render(<ThemePresetsSettings />);
    await waitFor(() => expect(listenThemePresetsChanged).toHaveBeenCalledOnce());

    act(() => onPresetsChanged());
    expect(await screen.findByText("1 of 50 saved")).toBeInTheDocument();

    await act(async () => {
      resolveInitial({
        used: 1,
        limit: 10,
        canCreate: true,
        planId: "free",
        planSource: "contract_default",
        freeLimit: 10,
      });
    });

    expect(screen.getByText("1 of 50 saved")).toBeInTheDocument();
    expect(screen.queryByText("1 of 10 saved")).not.toBeInTheDocument();
  });

  it("opens the apply dialog only after the appearance fingerprint is captured", async () => {
    let resolveFingerprint!: (value: string) => void;
    vi.mocked(api.getAppearanceFingerprint).mockReturnValueOnce(
      new Promise((resolve) => {
        resolveFingerprint = resolve;
      }),
    );

    render(<ThemePresetsSettings />);
    await waitForPresetRow();
    fireEvent.click(screen.getByRole("button", { name: "Apply Theme" }));

    expect(
      screen.queryByRole("alertdialog", { name: /Apply “Ocean”\?/ }),
    ).not.toBeInTheDocument();

    await act(async () => resolveFingerprint("fp-at-open"));
    expect(
      await screen.findByRole("alertdialog", { name: /Apply “Ocean”\?/ }),
    ).toBeInTheDocument();
  });

  it("uses the fingerprint captured at open time on confirm without refetching", async () => {
    render(<ThemePresetsSettings />);
    await waitForPresetRow();
    await openApplyDialog();
    vi.mocked(api.getAppearanceFingerprint).mockClear();

    const dialog = screen.getByRole("alertdialog", { name: /Apply “Ocean”\?/ });
    fireEvent.click(within(dialog).getByRole("button", { name: "Apply Theme" }));

    await waitFor(() => expect(applyThemePreset).toHaveBeenCalledTimes(1));
    expect(api.getAppearanceFingerprint).not.toHaveBeenCalled();
    expect(applyThemePreset).toHaveBeenCalledWith(
      "preset-ocean",
      3,
      "fp-test-ocean",
    );
  });

  it("refreshes the appearance fingerprint when Apply is reopened after a stale confirmation", async () => {
    applyThemePreset
      .mockRejectedValueOnce(
        new Error(
          "Your appearance changed since this confirmation opened. Review and try Apply again.",
        ),
      )
      .mockResolvedValueOnce(undefined);
    vi.mocked(api.getAppearanceFingerprint)
      .mockResolvedValueOnce("fp-stale-at-open")
      .mockResolvedValueOnce("fp-refreshed-at-reopen");

    render(<ThemePresetsSettings />);
    await waitForPresetRow();
    fireEvent.click(screen.getByRole("button", { name: "Apply Theme" }));
    const dialog = await screen.findByRole("alertdialog", { name: /Apply “Ocean”\?/ });

    fireEvent.click(within(dialog).getByRole("button", { name: "Apply Theme" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "Your appearance changed since this confirmation opened. Review and try Apply again.",
    );

    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() =>
      expect(
        screen.queryByRole("alertdialog", { name: /Apply “Ocean”\?/ }),
      ).not.toBeInTheDocument(),
    );

    fireEvent.click(screen.getByRole("button", { name: "Apply Theme" }));
    await screen.findByRole("alertdialog", { name: /Apply “Ocean”\?/ });
    fireEvent.click(
      within(screen.getByRole("alertdialog", { name: /Apply “Ocean”\?/ })).getByRole(
        "button",
        { name: "Apply Theme" },
      ),
    );

    await waitFor(() => expect(applyThemePreset).toHaveBeenCalledTimes(2));
    expect(applyThemePreset).toHaveBeenNthCalledWith(
      1,
      "preset-ocean",
      3,
      "fp-stale-at-open",
    );
    expect(applyThemePreset).toHaveBeenNthCalledWith(
      2,
      "preset-ocean",
      3,
      "fp-refreshed-at-reopen",
    );
    expect(api.getAppearanceFingerprint).toHaveBeenCalledTimes(2);
  });

  it("closes the overflow menu on Escape and outside pointerdown", async () => {
    render(<ThemePresetsSettings />);
    await waitForPresetRow();
    fireEvent.click(screen.getByRole("button", { name: "More actions for Ocean" }));
    expect(screen.getByRole("menuitem", { name: "Rename" })).toBeInTheDocument();

    fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
    await waitFor(() =>
      expect(screen.queryByRole("menuitem", { name: "Rename" })).not.toBeInTheDocument(),
    );

    fireEvent.click(screen.getByRole("button", { name: "More actions for Ocean" }));
    expect(screen.getByRole("menuitem", { name: "Rename" })).toBeInTheDocument();
    fireEvent.pointerDown(document.body);
    await waitFor(() =>
      expect(screen.queryByRole("menuitem", { name: "Rename" })).not.toBeInTheDocument(),
    );
  });

  it("keeps the latest Apply selection when fingerprint requests resolve out of order", async () => {
    let resolveOcean!: (fingerprint: string) => void;
    vi.mocked(api.listThemePresets).mockResolvedValueOnce([
      mockPreset(),
      mockPreset({ id: "preset-forest", name: "Forest", revision: 7 }),
    ]);
    vi.mocked(api.getAppearanceFingerprint)
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveOcean = resolve;
        }),
      )
      .mockResolvedValueOnce("fp-forest");

    render(<ThemePresetsSettings />);
    await waitFor(() =>
      expect(
        screen.getAllByRole("button", { name: "Apply Theme" }),
      ).toHaveLength(2),
    );
    const applyButtons = screen.getAllByRole("button", { name: "Apply Theme" });

    fireEvent.click(applyButtons[0]);
    fireEvent.click(applyButtons[1]);
    expect(
      await screen.findByRole("alertdialog", { name: /Apply “Forest”\?/ }),
    ).toBeInTheDocument();

    await act(async () => resolveOcean("fp-ocean"));

    expect(
      screen.getByRole("alertdialog", { name: /Apply “Forest”\?/ }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("alertdialog", { name: /Apply “Ocean”\?/ }),
    ).not.toBeInTheDocument();
  });
});
