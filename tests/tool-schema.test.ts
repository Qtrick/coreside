import { describe, expect, it } from "vitest";
import {
  isSupportedComponentType,
  validateAction,
  validateToolDefinition,
} from "@/lib/tool-schema";

describe("tool-schema", () => {
  it("accepts a valid tool definition", () => {
    const result = validateToolDefinition({
      id: "water-tracker",
      name: "Water Tracker",
      description: "Tracks glasses of water",
      layout: { type: "single-column" },
      components: [
        {
          id: "count",
          type: "counter",
          valueKey: "count",
          props: { label: "Glasses" },
        },
        {
          id: "add",
          type: "button",
          props: { label: "Add", variant: "primary" },
          actions: [{ type: "increment", target: "count", amount: 1 }],
        },
      ],
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.id).toBe("water-tracker");
      expect(result.data.components).toHaveLength(2);
    }
  });

  it("flags unsupported component types", () => {
    const result = validateToolDefinition({
      id: "bad-tool",
      name: "Bad Tool",
      components: [{ id: "x", type: "magicWidget" }],
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.warnings.some((w) => w.includes("unsupported"))).toBe(true);
    }
  });

  it("rejects malformed tool definitions", () => {
    const result = validateToolDefinition({ name: "Missing id" });
    expect(result.success).toBe(false);
  });

  it("validates actions", () => {
    expect(validateAction({ type: "toggle", target: "done" })).toEqual({
      type: "toggle",
      target: "done",
    });
    expect(validateAction({ type: "nope" })).toBeNull();
  });

  it("knows supported component types", () => {
    expect(isSupportedComponentType("quiz")).toBe(true);
    expect(isSupportedComponentType("iframe")).toBe(false);
  });
});
