import { describe, expect, it } from "vitest";
import {
  analyzeToolFit,
  DEFAULT_TOOL_VIEWPORT,
  parseToolViewportPolicy,
} from "./tool-viewport";

describe("tool viewport", () => {
  it("returns safe defaults for missing metadata", () => {
    expect(parseToolViewportPolicy(null)).toEqual(DEFAULT_TOOL_VIEWPORT);
  });

  it("parses nested viewportPolicy", () => {
    const policy = parseToolViewportPolicy({
      viewportPolicy: { minUsefulWidth: 600, preferredExpansionDirection: "left" },
    });
    expect(policy.minUsefulWidth).toBe(600);
    expect(policy.preferredExpansionDirection).toBe("left");
  });

  it("recommends expansion in smart mode when below minimum", () => {
    expect(
      analyzeToolFit({
        canvasWidth: 300,
        canvasHeight: 400,
        policy: DEFAULT_TOOL_VIEWPORT,
        layoutMode: "standard",
        adaptiveSizing: "smart",
        canAdjustSplit: false,
      }),
    ).toBe("native_expansion_recommended");
  });

  it("asks first when configured", () => {
    expect(
      analyzeToolFit({
        canvasWidth: 300,
        canvasHeight: 400,
        policy: DEFAULT_TOOL_VIEWPORT,
        layoutMode: "standard",
        adaptiveSizing: "ask",
        canAdjustSplit: false,
      }),
    ).toBe("native_expansion_requires_confirmation");
  });
});
