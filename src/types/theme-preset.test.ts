import { describe, expect, it } from "vitest";
import { DEFAULT_WALLPAPER } from "@/types/agent";
import {
  wallpaperLabelFromSnapshot,
  type AppearanceSnapshot,
} from "@/types/theme-preset";

function snapshot(
  overrides: Partial<AppearanceSnapshot> = {},
): AppearanceSnapshot {
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
    interfaceTransparency: 0,
    ...overrides,
  };
}

describe("wallpaperLabelFromSnapshot", () => {
  it("prefers wallpaperJson type with hyphens normalized to spaces", () => {
    const label = wallpaperLabelFromSnapshot(
      snapshot({
        wallpaperJson: JSON.stringify({ type: "image-cover", assetId: "a1" }),
        wallpaper: { kind: "matrix" },
      }),
    );
    expect(label).toBe("image cover");
  });

  it("falls back to wallpaper kind and maps none to No wallpaper", () => {
    expect(wallpaperLabelFromSnapshot(snapshot())).toBe("No wallpaper");
    expect(
      wallpaperLabelFromSnapshot(
        snapshot({ wallpaper: { kind: "aurora" }, wallpaperJson: "" }),
      ),
    ).toBe("aurora");
  });
});
