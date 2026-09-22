import { describe, expect, it } from "vitest";
import {
  clampInterfaceTransparency,
  computeInterfaceTransparencyTokens,
} from "./interface-transparency";

describe("interface transparency", () => {
  it("clamps to 0–60", () => {
    expect(clampInterfaceTransparency(-10)).toBe(0);
    expect(clampInterfaceTransparency(20)).toBe(20);
    expect(clampInterfaceTransparency(99)).toBe(60);
    expect(clampInterfaceTransparency("nope")).toBe(20);
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
    const balanced = computeInterfaceTransparencyTokens(20, {
      wallpaperActive: true,
    });
    const immersive = computeInterfaceTransparencyTokens(40, {
      wallpaperActive: true,
    });
    const max = computeInterfaceTransparencyTokens(60, {
      wallpaperActive: true,
    });
    expect(immersive.panelAlpha).toBeLessThan(solid.panelAlpha);
    expect(immersive.panelAlpha).toBeLessThan(balanced.panelAlpha);
    expect(max.panelAlpha).toBeLessThan(immersive.panelAlpha);
    expect(immersive.cardAlpha).toBeGreaterThan(immersive.panelAlpha);
    expect(immersive.controlAlpha).toBeGreaterThanOrEqual(immersive.cardAlpha);
    expect(immersive.modalAlpha).toBeGreaterThanOrEqual(0.9);
    // Nested cards must also become more translucent as the slider rises.
    expect(max.cardAlpha).toBeLessThan(balanced.cardAlpha);
    // RC3.3: at 20% ordinary cards must not clamp to fully opaque.
    expect(balanced.cardAlpha).toBeLessThan(1);
    expect(balanced.cardAlpha).toBeGreaterThan(0.8);
  });

  it("keeps nested surface alphas strictly above panel alpha when translucent", () => {
    const tokens = computeInterfaceTransparencyTokens(60, {
      wallpaperActive: true,
    });
    expect(tokens.cardAlpha).toBeGreaterThan(tokens.panelAlpha);
    expect(tokens.controlAlpha).toBeGreaterThan(tokens.panelAlpha);
    expect(tokens.modalAlpha).toBeGreaterThan(tokens.cardAlpha);
    expect(tokens.cardAlpha).toBeGreaterThanOrEqual(0.35);
    expect(tokens.panelAlpha).toBeLessThanOrEqual(0.55);
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
