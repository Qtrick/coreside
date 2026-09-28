import { describe, expect, it } from "vitest";
import { sanitizeInteractionPayload } from "@/lib/actions";
import { validateToolDefinition, isSupportedComponentType } from "@/lib/tool-schema";
import { ComponentTypeSchema, type ToolDefinition } from "@/types/tool";
import { StateContractSchema, SoftwareDocumentSchema } from "@/lib/software-document";

describe("adversarial style and script rejection", () => {
  it("rejects unknown and dangerous component types like styleTag, script, and iframe", () => {
    // 1. Direct schema check: dangerous component types are not in ComponentTypeSchema
    expect(ComponentTypeSchema.safeParse("styleTag").success).toBe(false);
    expect(ComponentTypeSchema.safeParse("script").success).toBe(false);
    expect(ComponentTypeSchema.safeParse("iframe").success).toBe(false);
    expect(ComponentTypeSchema.safeParse("<script>alert(1)</script>").success).toBe(false);
    expect(isSupportedComponentType("styleTag")).toBe(false);
    expect(isSupportedComponentType("script")).toBe(false);
    expect(isSupportedComponentType("iframe")).toBe(false);

    // 2. Tool definition validation flags unsupported component types in warnings
    const maliciousDoc: ToolDefinition = {
      id: "tool-malicious",
      name: "Attack Tool",
      description: "Tries to inject arbitrary elements",
      layout: { type: "single-column" },
      components: [
        {
          id: "evil-style",
          type: "styleTag",
          props: { css: "* { color: red !important; }" },
        },
        {
          id: "evil-script",
          type: "script",
          props: { src: "https://malicious-cdn.com/evil.js" },
        },
        {
          id: "evil-iframe",
          type: "iframe",
          props: { src: "https://evil.com" },
        },
      ],
    };

    const res = validateToolDefinition(maliciousDoc);
    expect(res.success).toBe(true);
    if (res.success) {
      expect(res.warnings.length).toBeGreaterThanOrEqual(3);
      expect(res.warnings.some((w) => w.includes("unsupported component type \"styleTag\""))).toBe(true);
      expect(res.warnings.some((w) => w.includes("unsupported component type \"script\""))).toBe(true);
      expect(res.warnings.some((w) => w.includes("unsupported component type \"iframe\""))).toBe(true);
    }
  });

  it("detects and rejects prototype pollution attempts in interaction payloads", () => {
    const payload = JSON.parse(
      '{"normalKey": "normalVal", "__proto__": {"polluted": true}, "constructor": {"prototype": {"admin": true}}}',
    );
    const result = sanitizeInteractionPayload(payload);
    // sanitizeInteractionPayload fails closed on __proto__ or constructor
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toMatch(/Disallowed dangerous property name/);
    }
    expect((Object.prototype as Record<string, unknown>).polluted).toBeUndefined();
    expect((Object.prototype as Record<string, unknown>).admin).toBeUndefined();
  });

  it("accepts valid interaction payloads within bounds and sanitizes clean objects", () => {
    const payload = {
      actionId: "btn-1",
      counter: 42,
      meta: { subkey: "hello" },
    };
    const result = sanitizeInteractionPayload(payload);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.data.actionId).toBe("btn-1");
      expect(result.data.counter).toBe(42);
      expect(result.data.meta).toEqual({ subkey: "hello" });
    }
  });

  it("rejects non-object or oversized interaction payloads", () => {
    // Non-object
    // @ts-expect-error testing invalid payload type
    expect(sanitizeInteractionPayload("not an object").ok).toBe(false);
    // @ts-expect-error testing invalid payload type
    expect(sanitizeInteractionPayload(null).ok).toBe(false);
    // @ts-expect-error testing invalid payload type
    expect(sanitizeInteractionPayload([1, 2, 3]).ok).toBe(false);

    // Oversized keys
    const bigPayload: Record<string, unknown> = {};
    for (let i = 0; i < 35; i++) {
      bigPayload[`key_${i}`] = i;
    }
    const result = sanitizeInteractionPayload(bigPayload);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toMatch(/exceeds maximum key limit/);
    }
  });

  it("flags duplicate component IDs in tool definitions", () => {
    const duplicateDoc: ToolDefinition = {
      id: "tool-dup",
      name: "Duplicate IDs",
      description: "Has identical component ids",
      layout: { type: "single-column" },
      components: [
        { id: "comp-1", type: "heading", props: { text: "Heading 1" } },
        { id: "comp-1", type: "heading", props: { text: "Heading 2" } },
      ],
    };
    const res = validateToolDefinition(duplicateDoc);
    expect(res.success).toBe(true);
    if (res.success) {
      expect(res.warnings.some((w) => w.includes("duplicate component id \"comp-1\""))).toBe(true);
    }
  });

  it("validates state contracts and software document schemas", () => {
    const validContract = {
      key: "userScore",
      type: "number",
      initialValue: 0,
      scope: "persistent",
    };
    expect(StateContractSchema.safeParse(validContract).success).toBe(true);

    const invalidContract = {
      key: "",
      type: "number",
    };
    expect(StateContractSchema.safeParse(invalidContract).success).toBe(false);

    const validDoc = {
      id: "doc-1",
      title: "My Software",
      sections: [],
      stateContracts: [validContract],
    };
    expect(SoftwareDocumentSchema.safeParse(validDoc).success).toBe(true);
  });
});

