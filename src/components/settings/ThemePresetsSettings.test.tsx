import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ThemePresetsSettings } from "./ThemePresetsSettings";
import { api } from "@/lib/tauri";
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
    saveCurrentThemePreset: vi.fn(),
    renameThemePreset: vi.fn(),
    deleteThemePreset: vi.fn(),
    updateThemePresetFromCurrent: vi.fn(),
  },
  listenThemePresetsChanged: vi.fn(async () => () => undefined),
  TauriCommandError: class TauriCommandError extends Error {
    code: string;
    constructor(message: string, code = "error") {
      super(message);
      this.code = code;
    }
  },
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

  it("opening the apply dialog does not call applyThemePreset", async () => {
    render(<ThemePresetsSettings />);
    await waitForPresetRow();

    fireEvent.click(screen.getByRole("button", { name: "Apply Theme" }));

    expect(
      screen.getByRole("alertdialog", { name: /Apply “Ocean”\?/ }),
    ).toBeInTheDocument();
    expect(applyThemePreset).not.toHaveBeenCalled();
  });

  it("focuses Cancel when the destructive apply dialog opens", async () => {
    render(<ThemePresetsSettings />);
    await waitForPresetRow();

    fireEvent.click(screen.getByRole("button", { name: "Apply Theme" }));
    const dialog = screen.getByRole("alertdialog", { name: /Apply “Ocean”\?/ });

    await waitFor(() => {
      expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    });
    expect(applyThemePreset).not.toHaveBeenCalled();
  });

  it("Cancel closes the apply dialog without calling applyThemePreset", async () => {
    render(<ThemePresetsSettings />);
    await waitForPresetRow();

    fireEvent.click(screen.getByRole("button", { name: "Apply Theme" }));
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

    fireEvent.click(screen.getByRole("button", { name: "Apply Theme" }));
    fireEvent.keyDown(document, { key: "Escape", code: "Escape" });

    await waitFor(() => {
      expect(
        screen.queryByRole("alertdialog", { name: /Apply “Ocean”\?/ }),
      ).not.toBeInTheDocument();
    });
    expect(applyThemePreset).not.toHaveBeenCalled();
  });

  it("confirm Apply calls applyThemePreset once with id and revision", async () => {
    render(<ThemePresetsSettings />);
    await waitForPresetRow();

    fireEvent.click(screen.getByRole("button", { name: "Apply Theme" }));
    const dialog = screen.getByRole("alertdialog", { name: /Apply “Ocean”\?/ });
    fireEvent.click(within(dialog).getByRole("button", { name: "Apply Theme" }));

    await waitFor(() => {
      expect(applyThemePreset).toHaveBeenCalledTimes(1);
    });
    expect(applyThemePreset).toHaveBeenCalledWith("preset-ocean", 3);
  });

  it("keeps a failed apply open and allows retry with the same revision", async () => {
    applyThemePreset
      .mockRejectedValueOnce(new Error("Theme preset was updated elsewhere"))
      .mockResolvedValueOnce(undefined);
    render(<ThemePresetsSettings />);
    await waitForPresetRow();

    fireEvent.click(screen.getByRole("button", { name: "Apply Theme" }));
    const dialog = screen.getByRole("alertdialog", { name: /Apply “Ocean”\?/ });
    fireEvent.click(within(dialog).getByRole("button", { name: "Apply Theme" }));

    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "Theme preset was updated elsewhere",
    );
    expect(dialog).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Apply Theme" }));

    await waitFor(() => expect(dialog).not.toBeInTheDocument());
    expect(applyThemePreset).toHaveBeenCalledTimes(2);
    expect(applyThemePreset).toHaveBeenNthCalledWith(1, "preset-ocean", 3);
    expect(applyThemePreset).toHaveBeenNthCalledWith(2, "preset-ocean", 3);
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

    fireEvent.click(screen.getByRole("button", { name: "Apply Theme" }));
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
});
