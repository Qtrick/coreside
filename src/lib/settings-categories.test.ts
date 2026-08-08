import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  DEFAULT_SETTINGS_CATEGORY,
  matchSettingsSearch,
  normalizeSettingsCategoryId,
  SETTINGS_CATEGORIES,
  SETTINGS_SEARCH_INDEX,
  settingsTargetDomId,
} from "./settings-categories";

describe("settings categories", () => {
  it("includes required consumer categories without wallpaper as a base category", () => {
    const ids = SETTINGS_CATEGORIES.map((c) => c.id);
    expect(ids).toContain("general");
    expect(ids).toContain("appearance");
    expect(ids).toContain("ai-access");
    expect(ids).toContain("agent");
    expect(ids).toContain("search");
    expect(ids).toContain("privacy");
    expect(ids).toContain("data");
    expect(ids).toContain("accessibility");
    expect(ids).toContain("advanced");
    expect(ids).toContain("help-learning");
    expect(ids).toContain("about");
    expect(ids).toContain("added");
    expect(ids).not.toContain("wallpaper");
    expect(SETTINGS_CATEGORIES.find((c) => c.id === "added")?.label).toBe(
      "App settings",
    );
  });

  it("normalizes invalid persisted category ids", () => {
    expect(normalizeSettingsCategoryId("not-a-category")).toBe(
      DEFAULT_SETTINGS_CATEGORY,
    );
    expect(normalizeSettingsCategoryId("ai-access")).toBe("ai-access");
  });

  it("routes search queries to the owning category", () => {
    expect(matchSettingsSearch("API key")[0]?.categoryId).toBe("ai-access");
    expect(matchSettingsSearch("privacy")[0]?.categoryId).toBe("privacy");
    expect(matchSettingsSearch("backup")[0]?.categoryId).toBe("data");
    expect(matchSettingsSearch("wallpaper")[0]?.categoryId).toBe("appearance");
    expect(matchSettingsSearch("tutorial")[0]?.categoryId).toBe("help-learning");
    expect(matchSettingsSearch("tour")[0]?.categoryId).toBe("help-learning");
    expect(normalizeSettingsCategoryId("wallpaper")).toBe("appearance");
    expect(normalizeSettingsCategoryId("wallpapers")).toBe("appearance");
    expect(matchSettingsSearch("recovery")[0]?.categoryId).toBe("advanced");
    expect(matchSettingsSearch("App permissions")[0]?.categoryId).toBe(
      "advanced",
    );
  });

  it("matches category names and descriptions", () => {
    expect(matchSettingsSearch("Everyday window")[0]?.categoryId).toBe(
      "general",
    );
    expect(matchSettingsSearch("Web research")[0]?.categoryId).toBe("search");
    expect(matchSettingsSearch("inclusive use")[0]?.categoryId).toBe(
      "accessibility",
    );
  });

  it("returns no hits for blank queries and scores token matches", () => {
    expect(matchSettingsSearch("   ")).toEqual([]);
    // Manual Dock selection is product-dormant — search must not surface it.
    expect(matchSettingsSearch("dock tile").some((h) => h.id === "dock-icon")).toBe(
      false,
    );
    expect(matchSettingsSearch("Classic Dark").some((h) => h.id === "dock-icon")).toBe(
      false,
    );
    expect(matchSettingsSearch("Split").some((h) => h.id === "dock-icon")).toBe(false);
    const wallpaper = matchSettingsSearch("wallpaper");
    expect(wallpaper[0]?.categoryId).toBe("appearance");
    expect(wallpaper[0]?.score).toBeGreaterThan(0);
  });

  it("keeps search index entries pointing at known categories", () => {
    const known = new Set(SETTINGS_CATEGORIES.map((c) => c.id));
    for (const entry of SETTINGS_SEARCH_INDEX) {
      expect(known.has(entry.categoryId)).toBe(true);
      expect(settingsTargetDomId(entry.id)).toBe(`settings-target-${entry.id}`);
    }
  });

  it("keeps settings CSS token contract for search shell radius", () => {
    const tokens = readFileSync(
      resolve(__dirname, "../styles/tokens.css"),
      "utf8",
    );
    expect(tokens).toMatch(/--radius-md:\s*var\(--radius\)/);
    expect(tokens).toMatch(/--text:\s*var\(--text-primary\)/);
    expect(tokens).toMatch(/--text-muted:\s*var\(--text-secondary\)/);
  });
});
