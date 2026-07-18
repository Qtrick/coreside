import { describe, expect, it } from "vitest";
import {
  contrastRatio,
  ensureReadableForeground,
  parseHexColor,
  relativeLuminance,
} from "@/lib/readability/contrast";

describe("parseHexColor", () => {
  it("parses 6-digit and 3-digit hex", () => {
    expect(parseHexColor("#000000")).toEqual({ r: 0, g: 0, b: 0 });
    expect(parseHexColor("#fff")).toEqual({ r: 255, g: 255, b: 255 });
    expect(parseHexColor("ffff00")).toEqual({ r: 255, g: 255, b: 0 });
  });

  it("rejects invalid values", () => {
    expect(parseHexColor("")).toBeNull();
    expect(parseHexColor("#gg0000")).toBeNull();
  });
});

describe("relativeLuminance", () => {
  it("is 0 for black and 1 for white", () => {
    expect(relativeLuminance("#000000")).toBeCloseTo(0, 5);
    expect(relativeLuminance("#ffffff")).toBeCloseTo(1, 5);
  });

  it("places yellow between black and white", () => {
    const yellow = relativeLuminance("#ffff00");
    expect(yellow).toBeGreaterThan(0.5);
    expect(yellow).toBeLessThan(1);
  });
});

describe("contrastRatio", () => {
  it("is 21 for black on white", () => {
    expect(contrastRatio("#000000", "#ffffff")).toBeCloseTo(21, 5);
    expect(contrastRatio("#ffffff", "#000000")).toBeCloseTo(21, 5);
  });

  it("is 1 for identical colors", () => {
    expect(contrastRatio("#ffff00", "#ffff00")).toBeCloseTo(1, 5);
  });

  it("gives strong contrast for yellow on black", () => {
    const ratio = contrastRatio("#ffff00", "#000000");
    expect(ratio).toBeGreaterThan(10);
    expect(ratio).toBeLessThan(21);
  });
});

describe("ensureReadableForeground", () => {
  it("lightens dark blue on a dark background", () => {
    const darkBlue = "#1a3a6e";
    const darkBg = "#141714";
    expect(contrastRatio(darkBlue, darkBg)).toBeLessThan(4.5);
    const fixed = ensureReadableForeground(darkBlue, darkBg, 4.5);
    expect(contrastRatio(fixed, darkBg)).toBeGreaterThanOrEqual(4.5);
    expect(relativeLuminance(fixed)).toBeGreaterThan(relativeLuminance(darkBlue));
  });

  it("darkens light blue on a light background", () => {
    const lightBlue = "#9ec9e8";
    const lightBg = "#f5f6f1";
    expect(contrastRatio(lightBlue, lightBg)).toBeLessThan(4.5);
    const fixed = ensureReadableForeground(lightBlue, lightBg, 4.5);
    expect(contrastRatio(fixed, lightBg)).toBeGreaterThanOrEqual(4.5);
    expect(relativeLuminance(fixed)).toBeLessThan(relativeLuminance(lightBlue));
  });

  it("leaves already-readable colors with sufficient contrast", () => {
    const fixed = ensureReadableForeground("#6ab0d4", "#141714", 4.5);
    expect(contrastRatio(fixed, "#141714")).toBeGreaterThanOrEqual(4.5);
  });
});
