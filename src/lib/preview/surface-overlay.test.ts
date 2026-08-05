import { describe, expect, it } from "vitest";
import {
  applyPreviewSurfaceOverlay,
  clearPreviewOverlaysForConversation,
  clearPreviewOverlaysMatching,
  clearPreviewSurfaceOverlays,
  getPreviewOverlayForTool,
  previewOverlayKey,
  type PreviewSurfaceOverlay,
} from "./surface-overlay";

const sampleDef = {
  id: "tool-1",
  name: "Counter",
  description: "test",
  components: [
    { id: "c1", type: "progress", props: { maximum: 10 } },
  ],
};

function sampleOverlay(
  overrides: Partial<PreviewSurfaceOverlay> = {},
): PreviewSurfaceOverlay {
  return {
    conversationId: "conv-1",
    turnId: "turn-1",
    toolId: "tool-1",
    surfaceId: "surf-tool-1",
    applicationId: "tool-1",
    tool: sampleDef as PreviewSurfaceOverlay["tool"],
    state: { n: 1 },
    revision: 2,
    sequence: 1,
    ...overrides,
  };
}

describe("preview surface overlay", () => {
  it("applies a validated preview paint", () => {
    const next = applyPreviewSurfaceOverlay(
      {},
      {
        conversationId: "conv-1",
        turnId: "turn-1",
        toolId: "tool-1",
        surfaceId: "surf-tool-1",
        definitionJson: sampleDef,
        stateJson: { n: 2 },
        revision: 3,
        sequence: 1,
      },
    );
    expect(previewOverlayKey({ toolId: "tool-1", surfaceId: "surf-tool-1" })).toBe(
      "tool-1",
    );
    expect(next["tool-1"]?.tool.components[0]?.props?.maximum).toBe(10);
    expect(next["tool-1"]?.state).toEqual({ n: 2 });
    expect(next["tool-1"]?.revision).toBe(3);
    expect(next["tool-1"]?.conversationId).toBe("conv-1");
  });

  it("ignores paint without conversationId", () => {
    const next = applyPreviewSurfaceOverlay(
      {},
      {
        conversationId: "",
        turnId: "turn-1",
        toolId: "tool-1",
        surfaceId: "surf-tool-1",
        definitionJson: sampleDef,
        stateJson: {},
        revision: 1,
        sequence: 1,
      },
    );
    expect(next).toEqual({});
  });

  it("ignores invalid definition json", () => {
    const next = applyPreviewSurfaceOverlay(
      {},
      {
        conversationId: "conv-1",
        turnId: "turn-1",
        surfaceId: "s1",
        definitionJson: { not: "a tool" },
        stateJson: {},
        revision: 1,
        sequence: 1,
      },
    );
    expect(next).toEqual({});
  });

  it("ignores stale sequence for the same turn", () => {
    const first = applyPreviewSurfaceOverlay(
      {},
      {
        conversationId: "conv-1",
        turnId: "turn-1",
        toolId: "tool-1",
        surfaceId: "surf-tool-1",
        definitionJson: sampleDef,
        stateJson: {},
        revision: 2,
        sequence: 2,
      },
    );
    const stale = applyPreviewSurfaceOverlay(first, {
      conversationId: "conv-1",
      turnId: "turn-1",
      toolId: "tool-1",
      surfaceId: "surf-tool-1",
      definitionJson: {
        ...sampleDef,
        components: [{ id: "c1", type: "progress", props: { maximum: 99 } }],
      },
      stateJson: {},
      revision: 1,
      sequence: 1,
    });
    expect(stale["tool-1"]?.revision).toBe(2);
    expect(stale["tool-1"]?.tool.components[0]?.props?.maximum).toBe(10);
  });

  it("clears all overlays", () => {
    const current = { "tool-1": sampleOverlay() };
    expect(clearPreviewSurfaceOverlays(current)).toEqual({});
  });

  it("clears overlays matching sync tool/surface ids", () => {
    const current = {
      "tool-1": sampleOverlay(),
      "tool-2": sampleOverlay({
        toolId: "tool-2",
        surfaceId: "surf-tool-2",
      }),
    };
    const next = clearPreviewOverlaysMatching(current, {
      conversationId: "conv-1",
      toolIds: ["tool-1"],
      surfaceIds: [],
    });
    expect(next["tool-1"]).toBeUndefined();
    expect(next["tool-2"]).toBeDefined();
  });

  it("clears by conversation when sync has no tool/surface ids", () => {
    const current = {
      "tool-1": sampleOverlay({ conversationId: "conv-a" }),
      "tool-2": sampleOverlay({
        conversationId: "conv-b",
        toolId: "tool-2",
        surfaceId: "surf-tool-2",
      }),
    };
    const next = clearPreviewOverlaysMatching(current, {
      conversationId: "conv-a",
      toolIds: [],
      surfaceIds: [],
    });
    expect(next["tool-1"]).toBeUndefined();
    expect(next["tool-2"]).toBeDefined();
  });

  it("clears conversation overlays on conflict-style empty targeting", () => {
    // Mirror applySyncOrConflictEvent conflict path: clear by conversationId.
    const current = {
      "tool-1": sampleOverlay({ conversationId: "conv-a" }),
      "tool-2": sampleOverlay({
        conversationId: "conv-b",
        toolId: "tool-2",
        surfaceId: "surf-tool-2",
      }),
    };
    const next = clearPreviewOverlaysMatching(current, {
      conversationId: "conv-a",
      toolIds: [],
      surfaceIds: [],
    });
    expect(Object.keys(next)).toEqual(["tool-2"]);
  });

  it("does not clear-all on unscoped empty targeting", () => {
    const current = { "tool-1": sampleOverlay() };
    expect(
      clearPreviewOverlaysMatching(current, { toolIds: [], surfaceIds: [] }),
    ).toBe(current);
  });

  it("clears only one conversation on interrupt", () => {
    const current = {
      "tool-1": sampleOverlay({ conversationId: "conv-a" }),
      "tool-2": sampleOverlay({
        conversationId: "conv-b",
        toolId: "tool-2",
        surfaceId: "surf-tool-2",
      }),
    };
    const next = clearPreviewOverlaysForConversation(current, "conv-a");
    expect(next["tool-1"]).toBeUndefined();
    expect(next["tool-2"]).toBeDefined();
  });

  it("does not clear by tool id without conversation scope", () => {
    const current = {
      "tool-1": sampleOverlay({ conversationId: "conv-a" }),
    };
    const next = clearPreviewOverlaysMatching(current, {
      toolIds: ["tool-1"],
      surfaceIds: [],
    });
    expect(next).toBe(current);
  });

  it("does not clear overlay from another conversation on tool id match", () => {
    const current = {
      "tool-1": sampleOverlay({ conversationId: "conv-a" }),
    };
    const next = clearPreviewOverlaysMatching(current, {
      conversationId: "conv-b",
      toolIds: ["tool-1"],
      surfaceIds: [],
    });
    expect(next["tool-1"]).toBeDefined();
  });

  it("clears scoped overlay when conversation and tool id match", () => {
    const current = {
      "tool-1": sampleOverlay({ conversationId: "conv-b" }),
    };
    const next = clearPreviewOverlaysMatching(current, {
      conversationId: "conv-b",
      toolIds: ["tool-1"],
      surfaceIds: [],
    });
    expect(next["tool-1"]).toBeUndefined();
  });

  it("resolves overlay for active tool only when conversation matches", () => {
    const overlays = { "tool-1": sampleOverlay({ conversationId: "conv-1" }) };
    expect(getPreviewOverlayForTool(overlays, "tool-1", "conv-1")?.surfaceId).toBe(
      "surf-tool-1",
    );
    expect(getPreviewOverlayForTool(overlays, "tool-1", "conv-other")).toBeNull();
    expect(getPreviewOverlayForTool(overlays, "missing", "conv-1")).toBeNull();
  });
});
