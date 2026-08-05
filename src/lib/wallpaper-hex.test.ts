import { describe, expect, it } from "vitest";
import {
  isCanonicalHexColor,
  normalizeCanonicalHex,
} from "./wallpaper-hex";
import {
  cssFilterFromConfig,
  WallpaperFilterConfigSchema,
} from "./wallpaper-filter";

describe("wallpaper hex", () => {
  it("accepts canonical #RRGGBB and expands #RGB", () => {
    expect(isCanonicalHexColor("#33ff66")).toBe(true);
    expect(isCanonicalHexColor("#ABC")).toBe(false);
    expect(normalizeCanonicalHex("#ABC")).toBe("#aabbcc");
    expect(normalizeCanonicalHex("#33FF66")).toBe("#33ff66");
    expect(normalizeCanonicalHex("red")).toBeNull();
    expect(normalizeCanonicalHex("#33ff66ff")).toBeNull();
  });
});

describe("wallpaper filter", () => {
  it("rejects out-of-range numeric fields", () => {
    expect(WallpaperFilterConfigSchema.safeParse({ brightness: 3 }).success).toBe(
      false,
    );
    expect(WallpaperFilterConfigSchema.safeParse({ blurPx: 40 }).success).toBe(
      false,
    );
  });

  it("builds CSS filter only from typed fields", () => {
    expect(cssFilterFromConfig({ preset: "none" })).toBeUndefined();
    expect(cssFilterFromConfig({ preset: "vivid" })).toMatch(/saturate/);
    expect(cssFilterFromConfig({ blurPx: 4 })).toBe("blur(4px)");
  });
});
