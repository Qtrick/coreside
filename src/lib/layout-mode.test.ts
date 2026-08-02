import { describe, expect, it } from "vitest";
import {
  classifyLayoutMode,
  clampSplitForWidth,
  clampSplitRatio,
} from "./layout-mode";

describe("layout mode", () => {
  it("clamps split ratio", () => {
    expect(clampSplitRatio(0.1)).toBe(0.28);
    expect(clampSplitRatio(0.9)).toBe(0.72);
    expect(clampSplitRatio(0.5)).toBe(0.5);
  });

  it("uses compact when main width cannot host both panes", () => {
    expect(
      classifyLayoutMode({
        shellWidth: 900,
        mainWidth: 600,
        sidebarWidth: 80,
        toolOpen: true,
        splitRatio: 0.5,
      }),
    ).toBe("compact");
  });

  it("uses wide when both panes fit", () => {
    expect(
      classifyLayoutMode({
        shellWidth: 1440,
        mainWidth: 1100,
        sidebarWidth: 272,
        toolOpen: true,
        splitRatio: 0.5,
      }),
    ).toBe("wide");
  });

  it("clamps split so minima fit", () => {
    const r = clampSplitForWidth(0.5, 800);
    expect(800 * r).toBeGreaterThanOrEqual(320 - 1);
    expect(800 * (1 - r)).toBeGreaterThanOrEqual(360 - 1);
  });
});
