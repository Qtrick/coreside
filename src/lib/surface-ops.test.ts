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

describe("safeDuplicateComponent", () => {
  it("regenerates IDs recursively and disambiguates input valueKey and preservationKey", async () => {
    const { safeDuplicateComponent } = await import("./surface-ops");
    const original: ToolComponent = {
      id: "card-root",
      type: "card",
      props: { title: "Original Card" },
      children: [
        {
          id: "input-title",
          type: "textInput",
          props: {
            label: "Title",
            valueKey: "taskTitle",
            preservationKey: "focus:taskTitle",
          },
        },
        {
          id: "table-data",
          type: "dataTable",
          props: {
            rowsKey: "sharedTasks",
            columns: [{ key: "title", label: "Title" }],
          },
        },
      ],
    };

    const duplicate = safeDuplicateComponent(original, "dup1");

    // Root has fresh ID
    expect(duplicate.id).not.toBe("card-root");
    expect(duplicate.id).toContain("dup1");

    // Children have fresh IDs
    expect(duplicate.children).toHaveLength(2);
    expect(duplicate.children![0].id).not.toBe("input-title");
    expect(duplicate.children![1].id).not.toBe("table-data");
    expect(duplicate.children![0].id).not.toBe(duplicate.children![1].id);

    // Input valueKey is disambiguated so state does not collide
    expect(duplicate.children![0].props?.valueKey).toBe("taskTitle_copy_dup1");
    // Preservation key is isolated so focus does not jump
    expect(duplicate.children![0].props?.preservationKey).toBe("focus:taskTitle_copy_dup1");

    // Read-only data collection rowsKey is preserved so data still displays
    expect(duplicate.children![1].props?.rowsKey).toBe("sharedTasks");
  });
});

describe("makeRemoveComponentOp & makeMoveComponentOp", () => {
  it("creates valid AppOperation structures with required targeting", async () => {
    const { makeRemoveComponentOp, makeMoveComponentOp } = await import("./surface-ops");
    const removeOp = makeRemoveComponentOp({
      surfaceId: "surf-1",
      componentId: "comp-a",
      baseRevision: 3,
    });
    expect(removeOp.type).toBe("component.remove");
    expect(removeOp.target.surfaceId).toBe("surf-1");
    expect(removeOp.target.componentId).toBe("comp-a");
    expect(removeOp.baseRevision).toBe(3);

    const moveOp = makeMoveComponentOp({
      surfaceId: "surf-1",
      componentId: "comp-b",
      parentId: "parent-x",
      index: 2,
      baseRevision: 4,
    });
    expect(moveOp.type).toBe("component.move");
    expect(moveOp.target.componentId).toBe("comp-b");
    expect(moveOp.target.parentId).toBe("parent-x");
    expect(moveOp.payload).toEqual({ index: 2 });
    expect(moveOp.baseRevision).toBe(4);
  });
});
