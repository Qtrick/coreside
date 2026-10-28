import fs from "node:fs";
import path from "node:path";
import { beforeEach, describe, expect, it } from "vitest";
import {
  DEFAULT_ACCENTS,
  DEFAULT_APPEARANCE,
  appearanceFromSettings,
  applyAppearanceCssVars,
} from "@/stores/app-store";
import { api, __resetMockDb } from "@/lib/tauri";

describe("monochrome theme defaults and customization preservation", () => {
  beforeEach(async () => {
    await __resetMockDb();
    document.documentElement.removeAttribute("style");
  });

  describe("TypeScript defaults (app-store.ts)", () => {
    it("defines monochrome default accents: #171717 (light) and #fafafa (dark), secondary #737373 and #a3a3a3", () => {
      expect(DEFAULT_ACCENTS.accentPrimaryLight).toBe("#171717");
      expect(DEFAULT_ACCENTS.accentPrimaryDark).toBe("#fafafa");
      expect(DEFAULT_ACCENTS.accentSecondaryLight).toBe("#737373");
      expect(DEFAULT_ACCENTS.accentSecondaryDark).toBe("#a3a3a3");
    });

    it("includes monochrome accents in DEFAULT_APPEARANCE", () => {
      expect(DEFAULT_APPEARANCE.accentPrimaryLight).toBe("#171717");
      expect(DEFAULT_APPEARANCE.accentPrimaryDark).toBe("#fafafa");
      expect(DEFAULT_APPEARANCE.accentSecondaryLight).toBe("#737373");
      expect(DEFAULT_APPEARANCE.accentSecondaryDark).toBe("#a3a3a3");
      expect(DEFAULT_APPEARANCE.backgroundLight).toBe("#f5f5f5");
      expect(DEFAULT_APPEARANCE.backgroundDark).toBe("#0a0a0a");
      expect(DEFAULT_APPEARANCE.surfaceLight).toBe("#ffffff");
      expect(DEFAULT_APPEARANCE.surfaceDark).toBe("#171717");
    });
  });

  describe("Mock settings defaults", () => {
    it("returns monochrome accent defaults from mock get_settings", async () => {
      const settings = await api.getSettings();
      expect(settings.accentPrimaryLight).toBe("#171717");
      expect(settings.accentPrimaryDark).toBe("#fafafa");
      expect(settings.accentSecondaryLight).toBe("#737373");
      expect(settings.accentSecondaryDark).toBe("#a3a3a3");
    });
  });

  describe("tokens.css baseline", () => {
    it("declares #171717 / #737373 in :root and #fafafa / #a3a3a3 in html[data-theme='dark']", () => {
      const cssPath = path.resolve(__dirname, "../styles/tokens.css");
      const css = fs.readFileSync(cssPath, "utf-8");

      // Extract :root block
      const rootMatch = css.match(/:root\s*\{([^}]+)\}/);
      expect(rootMatch).not.toBeNull();
      const rootContent = rootMatch![1];
      expect(rootContent).toMatch(/--accent-primary:\s*#171717;/);
      expect(rootContent).toMatch(/--accent-secondary:\s*#737373;/);
      expect(rootContent).toMatch(/--text-primary:\s*#171717;/);
      expect(rootContent).toMatch(/--text-secondary:\s*#737373;/);

      // Extract html[data-theme="dark"] block
      const darkMatch = css.match(/html\[data-theme="dark"\]\s*\{([^}]+)\}/);
      expect(darkMatch).not.toBeNull();
      const darkContent = darkMatch![1];
      expect(darkContent).toMatch(/--accent-primary:\s*#fafafa;/);
      expect(darkContent).toMatch(/--accent-secondary:\s*#a3a3a3;/);
      expect(darkContent).toMatch(/--text-primary:\s*#fafafa;/);
      expect(darkContent).toMatch(/--text-secondary:\s*#a3a3a3;/);
    });
  });

  describe("Custom accent preservation", () => {
    it("preserves fully customized user accents without overriding with defaults", () => {
      const customBlueAndGreen = {
        accentPrimaryLight: "#2563eb",
        accentPrimaryDark: "#60a5fa",
        accentSecondaryLight: "#16a34a",
        accentSecondaryDark: "#4ade80",
      };

      const resolved = appearanceFromSettings(customBlueAndGreen);

      expect(resolved.accentPrimaryLight).toBe("#2563eb");
      expect(resolved.accentPrimaryDark).toBe("#60a5fa");
      expect(resolved.accentSecondaryLight).toBe("#16a34a");
      expect(resolved.accentSecondaryDark).toBe("#4ade80");
    });

    it("preserves partial user accent customizations and falls back cleanly for unconfigured fields", () => {
      const partialCustom = {
        accentPrimaryLight: "#059669", // custom emerald
      };

      const resolved = appearanceFromSettings(partialCustom);

      expect(resolved.accentPrimaryLight).toBe("#059669");
      expect(resolved.accentPrimaryDark).toBe(DEFAULT_ACCENTS.accentPrimaryDark);
      expect(resolved.accentSecondaryLight).toBe(DEFAULT_ACCENTS.accentSecondaryLight);
      expect(resolved.accentSecondaryDark).toBe(DEFAULT_ACCENTS.accentSecondaryDark);
    });

    it("applies customized accents to CSS variables while respecting contrast", () => {
      const customPalette = appearanceFromSettings({
        accentPrimaryLight: "#2563eb",
        accentPrimaryDark: "#60a5fa",
        accentSecondaryLight: "#7c3aed",
        accentSecondaryDark: "#a78bfa",
      });

      applyAppearanceCssVars("light", customPalette);

      const root = document.documentElement;
      const primaryLight = root.style.getPropertyValue("--accent-primary");
      const secondaryLight = root.style.getPropertyValue("--accent-secondary");
      expect(primaryLight).toBeTruthy();
      expect(secondaryLight).toBeTruthy();

      applyAppearanceCssVars("dark", customPalette);
      const primaryDark = root.style.getPropertyValue("--accent-primary");
      const secondaryDark = root.style.getPropertyValue("--accent-secondary");
      expect(primaryDark).toBeTruthy();
      expect(secondaryDark).toBeTruthy();
      // Ensure dark theme resolves differently than light theme
      expect(primaryDark).not.toBe(primaryLight);
    });
  });
});
