import { describe, expect, it } from "vitest";
import { mergeStateForDefinitionPatch } from "./surface-ops";
import type { ToolComponent } from "@/types/tool";

describe("mergeStateForDefinitionPatch", () => {
  it("preserves state when compatible types remain", () => {
    const previous: ToolComponent[] = [
      { id: "field", type: "text_input", props: { value: "" } },
    ];
    const next: ToolComponent[] = [
      { id: "field", type: "text_input", props: { label: "Name" } },
    ];
    const merged = mergeStateForDefinitionPatch({
      previousComponents: previous,
      nextComponents: next,
      previousState: { field: "typed" },
      hydratedState: {},
    });
    expect(merged.field).toBe("typed");
  });

  it("drops state when types are incompatible under default policy", () => {
    const previous: ToolComponent[] = [
      { id: "field", type: "text_input", props: {} },
    ];
    const next: ToolComponent[] = [
      { id: "field", type: "heading", props: { text: "Hi" } },
    ];
    const merged = mergeStateForDefinitionPatch({
      previousComponents: previous,
      nextComponents: next,
      previousState: { field: "typed" },
      hydratedState: {},
    });
    expect(merged.field).toBeUndefined();
  });

  it("drops state when policy is reset_explicitly", () => {
    const previous: ToolComponent[] = [
      { id: "field", type: "text_input", props: {} },
    ];
    const next: ToolComponent[] = [
      {
        id: "field",
        type: "text_input",
        props: { preservationPolicy: "reset_explicitly" },
      },
    ];
    const merged = mergeStateForDefinitionPatch({
      previousComponents: previous,
      nextComponents: next,
      previousState: { field: "typed" },
      hydratedState: {},
    });
    expect(merged.field).toBeUndefined();
  });
});
