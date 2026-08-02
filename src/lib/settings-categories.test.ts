import { describe, expect, it } from "vitest";
import {
  DEFAULT_SETTINGS_CATEGORY,
  matchSettingsSearch,
  normalizeSettingsCategoryId,
  SETTINGS_CATEGORIES,
  SETTINGS_SEARCH_INDEX,
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
    expect(ids).toContain("about");
    expect(ids).toContain("added");
    expect(ids).not.toContain("wallpaper");
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
    expect(matchSettingsSearch("wallpaper")[0]?.categoryId).toBe("added");
    expect(matchSettingsSearch("recovery")[0]?.categoryId).toBe("advanced");
  });

  it("returns no hits for blank queries and scores token matches", () => {
    expect(matchSettingsSearch("   ")).toEqual([]);
    const hits = matchSettingsSearch("dock tile");
    expect(hits[0]?.id).toBe("dock-icon");
    expect(hits[0]?.score).toBeGreaterThan(0);
  });

  it("keeps search index entries pointing at known categories", () => {
    const known = new Set(SETTINGS_CATEGORIES.map((c) => c.id));
    for (const entry of SETTINGS_SEARCH_INDEX) {
      expect(known.has(entry.categoryId)).toBe(true);
    }
  });
});
