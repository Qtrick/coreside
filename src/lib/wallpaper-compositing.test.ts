/**
 * Deterministic wallpaper compositing proof helpers.
 * Pixel-perfect Canvas tests use a seeded static fixture; these unit tests
 * prove the alpha mapping is monotonic (required for visible transparency).
 */

import { describe, expect, it } from "vitest";
import {
  applyInterfaceTransparencyCssVars,
  computeInterfaceTransparencyTokens,
} from "./interface-transparency";
import {
  assertMonotonicWallpaperVisibility,
  sampleWallpaperThroughPanel,
} from "./wallpaper-visual-proof";

describe("wallpaper compositing proof", () => {
  it("panel and card alphas decrease monotonically across presets", () => {
    const levels = [0, 35, 70, 100].map((v) =>
      computeInterfaceTransparencyTokens(v, { wallpaperActive: true }),
    );
    for (let i = 1; i < levels.length; i++) {
      expect(levels[i]!.panelAlpha).toBeLessThan(levels[i - 1]!.panelAlpha);
      expect(levels[i]!.cardAlpha).toBeLessThanOrEqual(levels[i - 1]!.cardAlpha);
      expect(levels[i]!.sidebarAlpha).toBeLessThan(levels[i - 1]!.sidebarAlpha);
    }
    // At max transparency, panels must be clearly translucent.
    expect(levels[3]!.panelAlpha).toBeLessThan(0.15);
    expect(levels[3]!.cardAlpha).toBeLessThanOrEqual(0.40);
  });

  it("writes nested overlay CSS variables, not only panel overlays", () => {
    const tokens = computeInterfaceTransparencyTokens(40, {
      wallpaperActive: true,
    });
    applyInterfaceTransparencyCssVars(tokens);
    const root = document.documentElement;
    expect(root.style.getPropertyValue("--core-card-overlay")).toMatch(
      /color-mix/,
    );
    expect(root.style.getPropertyValue("--core-control-overlay")).toMatch(
      /color-mix/,
    );
    expect(root.style.getPropertyValue("--core-muted-overlay")).toMatch(
      /color-mix/,
    );
    expect(root.style.getPropertyValue("--core-modal-overlay")).toMatch(
      /color-mix/,
    );
    expect(root.style.getPropertyValue("--core-backdrop-blur")).toMatch(/px$/);
  });

  it("sets backdrop blur to 0px at maximum transparency", () => {
    const tokens = computeInterfaceTransparencyTokens(100, {
      wallpaperActive: true,
    });
    applyInterfaceTransparencyCssVars(tokens);
    expect(
      document.documentElement.style.getPropertyValue("--core-backdrop-blur"),
    ).toBe("0px");
  });

  it("forces solid overlays when wallpaper is inactive even if preference is high", () => {
    const tokens = computeInterfaceTransparencyTokens(60, {
      wallpaperActive: false,
    });
    applyInterfaceTransparencyCssVars(tokens);
    expect(
      document.documentElement.style.getPropertyValue("--core-content-overlay"),
    ).toMatch(/100%/);
    // Blur var collapses; base .sidebar keeps a fixed blur so chrome is unchanged.
    expect(
      document.documentElement.style.getPropertyValue("--core-backdrop-blur"),
    ).toBe("0px");
  });

  it("proves monotonic wallpaper pixel contribution through panels (visual model)", () => {
    const samples = assertMonotonicWallpaperVisibility();
    expect(samples.length).toBe(8);
    expect(samples[0]!.wallpaperContribution).toBe(0);
    expect(samples[samples.length - 1]!.preference).toBe(100);
    expect(samples[samples.length - 1]!.wallpaperContribution).toBeGreaterThanOrEqual(0.9);
    expect(sampleWallpaperThroughPanel(0).blended).toEqual({
      r: 245,
      g: 245,
      b: 242,
    });
  });
});
