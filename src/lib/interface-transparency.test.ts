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
    const immersive = computeInterfaceTransparencyTokens(40, {
      wallpaperActive: true,
    });
    expect(immersive.panelAlpha).toBeLessThan(solid.panelAlpha);
    expect(immersive.cardAlpha).toBeGreaterThan(immersive.panelAlpha);
    expect(immersive.modalAlpha).toBeGreaterThanOrEqual(0.94);
  });
});
