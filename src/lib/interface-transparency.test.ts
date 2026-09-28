import { describe, expect, it } from "vitest";
import {
  clampInterfaceTransparency,
  computeInterfaceTransparencyTokens,
} from "./interface-transparency";

describe("interface transparency", () => {
  it("clamps to 0–100", () => {
    expect(clampInterfaceTransparency(-10)).toBe(0);
    expect(clampInterfaceTransparency(20)).toBe(20);
    expect(clampInterfaceTransparency(150)).toBe(100);
    expect(clampInterfaceTransparency(99)).toBe(99);
    expect(clampInterfaceTransparency("nope")).toBe(35);
  });

  it("keeps solid surfaces when no wallpaper is active", () => {
    const tokens = computeInterfaceTransparencyTokens(40, {
      wallpaperActive: false,
    });
    expect(tokens.effective).toBe(0);
    expect(tokens.panelAlpha).toBe(1);
    expect(tokens.preference).toBe(40);
  });

  it("lowers panel alpha as transparency rises, keeping stronger cards", () => {
    const solid = computeInterfaceTransparencyTokens(0, {
      wallpaperActive: true,
    });
    const balanced = computeInterfaceTransparencyTokens(35, {
      wallpaperActive: true,
    });
    const immersive = computeInterfaceTransparencyTokens(70, {
      wallpaperActive: true,
    });
    const max = computeInterfaceTransparencyTokens(100, {
      wallpaperActive: true,
    });
    expect(immersive.panelAlpha).toBeLessThan(solid.panelAlpha);
    expect(immersive.panelAlpha).toBeLessThan(balanced.panelAlpha);
    expect(max.panelAlpha).toBeLessThan(immersive.panelAlpha);
    expect(immersive.cardAlpha).toBeGreaterThan(immersive.panelAlpha);
    expect(immersive.controlAlpha).toBeGreaterThanOrEqual(immersive.cardAlpha);
    expect(immersive.modalAlpha).toBeGreaterThanOrEqual(0.50);
    // Nested cards must also become more translucent as the slider rises.
    expect(max.cardAlpha).toBeLessThan(balanced.cardAlpha);
    // at 35% ordinary cards must not clamp to fully opaque.
    expect(balanced.cardAlpha).toBeLessThan(1);
    expect(balanced.cardAlpha).toBeGreaterThan(0.5);
  });

  it("keeps nested surface alphas strictly above panel alpha when translucent", () => {
    const tokens = computeInterfaceTransparencyTokens(100, {
      wallpaperActive: true,
    });
    expect(tokens.cardAlpha).toBeGreaterThan(tokens.panelAlpha);
    expect(tokens.controlAlpha).toBeGreaterThan(tokens.panelAlpha);
    expect(tokens.modalAlpha).toBeGreaterThan(tokens.cardAlpha);
    // At 100% max transparency, cards drop to translucent glass (~0.08) rather than heavy 30% blocks
    expect(tokens.cardAlpha).toBeGreaterThanOrEqual(0.08);
    expect(tokens.cardAlpha).toBeLessThan(0.20);
    expect(tokens.panelAlpha).toBeLessThanOrEqual(0.05);
  });

  it("produces strictly monotonic alphas across all 8 inspection levels (0, 10, 25, 40, 50, 70, 85, 100)", () => {
    const levels = [0, 10, 25, 40, 50, 70, 85, 100];
    const results = levels.map((lvl) =>
      computeInterfaceTransparencyTokens(lvl, { wallpaperActive: true }),
    );

    for (let i = 1; i < results.length; i++) {
      const prev = results[i - 1]!;
      const cur = results[i]!;
      expect(cur.panelAlpha).toBeLessThan(prev.panelAlpha);
      expect(cur.cardAlpha).toBeLessThan(prev.cardAlpha);
      expect(cur.controlAlpha).toBeLessThan(prev.controlAlpha);
      expect(cur.sidebarAlpha).toBeLessThan(prev.sidebarAlpha);
      expect(cur.headerAlpha).toBeLessThan(prev.headerAlpha);
      expect(cur.modalAlpha).toBeLessThan(prev.modalAlpha);
    }

    // Level 0: fully opaque
    expect(results[0]!.panelAlpha).toBe(1);
    expect(results[0]!.cardAlpha).toBe(1);

    // Level 100: maximum transparency (wallpaper show-through target)
    const max = results[results.length - 1]!;
    expect(max.panelAlpha).toBe(0.02);
    expect(max.cardAlpha).toBe(0.08);
    expect(max.controlAlpha).toBe(0.18);
    expect(max.modalAlpha).toBe(0.45);
    expect(max.backdropBlurPx).toBe(0);
    expect(results[0]!.backdropBlurPx).toBe(0);
    // Mid transparency keeps some blur; it must fall as transparency rises.
    expect(results[2]!.backdropBlurPx).toBeGreaterThan(results[5]!.backdropBlurPx);
  });

  it("collapses backdrop blur toward zero as interface transparency rises", () => {
    const solid = computeInterfaceTransparencyTokens(0, { wallpaperActive: true });
    const balanced = computeInterfaceTransparencyTokens(35, { wallpaperActive: true });
    const immersive = computeInterfaceTransparencyTokens(70, { wallpaperActive: true });
    const max = computeInterfaceTransparencyTokens(100, { wallpaperActive: true });
    expect(solid.backdropBlurPx).toBe(0);
    expect(balanced.backdropBlurPx).toBeGreaterThan(max.backdropBlurPx);
    expect(immersive.backdropBlurPx).toBeGreaterThan(max.backdropBlurPx);
    expect(max.backdropBlurPx).toBe(0);
  });

  it("keeps Solid (0%) fully opaque including sidebar", () => {
    const solid = computeInterfaceTransparencyTokens(0, {
      wallpaperActive: true,
    });
    expect(solid.panelAlpha).toBe(1);
    expect(solid.sidebarAlpha).toBe(1);
    expect(solid.cardAlpha).toBe(1);
    expect(solid.controlAlpha).toBe(1);
    expect(solid.modalAlpha).toBe(1);
  });
});
