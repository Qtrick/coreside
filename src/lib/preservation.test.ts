import { describe, expect, it, vi } from "vitest";
import {
  shouldPreserveComponent,
  captureScrollSnapshot,
  restoreScrollSnapshot,
  captureFocusSnapshot,
  restoreFocusSnapshot,
  reconcileDirtyOverCanonical,
  inferBaselineResetKeys,
  mergeDirtyReconcileResetKeys,
  resolvePendingInteractionIdempotencyKey,
  runRendererMountPatchFlush,
  startRendererMountLifecycle,
} from "./preservation";
import {
  canNavigateBack,
  canNavigateForward,
  isRouteNavigationNoOp,
} from "./route-navigation";

describe("shouldPreserveComponent", () => {
  it("rejects replace and reset_explicitly", () => {
    expect(shouldPreserveComponent("replace", "text", "text")).toBe(false);
    expect(shouldPreserveComponent("reset_explicitly", "text", "heading")).toBe(
      false,
    );
  });

  it("preserves focus regardless of type change", () => {
    expect(shouldPreserveComponent("preserve_focus", "text", "heading")).toBe(
      true,
    );
  });

  it("preserve_if_compatible requires matching types", () => {
    expect(shouldPreserveComponent("preserve_if_compatible", "text", "text")).toBe(
      true,
    );
    expect(
      shouldPreserveComponent("preserve_if_compatible", "text", "heading"),
    ).toBe(false);
  });
});

describe("reconcileDirtyOverCanonical", () => {
  it("keeps dirty typing over canonical while applying unrelated keys", () => {
    const next = reconcileDirtyOverCanonical(
      { title: "server", filter: "all", count: 2 },
      { title: "user-typing" },
    );
    expect(next).toEqual({ title: "user-typing", filter: "all", count: 2 });
  });

  it("honors explicit reset keys", () => {
    const next = reconcileDirtyOverCanonical(
      { title: "", notes: "kept" },
      { title: "stale-dirty", notes: "local" },
      ["title"],
    );
    expect(next.title).toBe("");
    expect(next.notes).toBe("local");
  });

  it("preserves empty-string dirty values (cleared field mid-typing)", () => {
    const next = reconcileDirtyOverCanonical(
      { title: "server-title", notes: "n" },
      { title: "" },
    );
    expect(next.title).toBe("");
    expect(next.notes).toBe("n");
  });

  it("trims blank reset keys and leaves other dirty keys intact", () => {
    const next = reconcileDirtyOverCanonical(
      { a: 1, b: 2, c: 3 },
      { a: 9, b: 8, c: 7 },
      [" a ", "", "b"],
    );
    expect(next).toEqual({ a: 1, b: 2, c: 7 });
  });

  it("skips overlay when server diverged from last persist (reset inference)", () => {
    const lastPersisted: Record<string, unknown> = { title: "hel", filter: "all" };
    const canonical: Record<string, unknown> = { filter: "all" };
    const dirty = { title: "hello" };
    const resetKeys = inferBaselineResetKeys(lastPersisted, canonical);
    const next = reconcileDirtyOverCanonical(canonical, dirty, resetKeys);
    expect(next.title).toBeUndefined();
    expect(next.filter).toBe("all");
  });

  it("keeps never-persisted dirty when server injects a new default without baseline", () => {
    const lastPersisted: Record<string, unknown> = {};
    const canonical: Record<string, unknown> = { title: "agent-default" };
    const dirty = { title: "user-typing" };
    const resetKeys = inferBaselineResetKeys(lastPersisted, canonical);
    const next = reconcileDirtyOverCanonical(canonical, dirty, resetKeys);
    expect(next.title).toBe("user-typing");
  });

  it("honors explicit reset key even when never persisted", () => {
    const next = reconcileDirtyOverCanonical(
      { title: "" },
      { title: "stale-dirty" },
      ["title"],
    );
    expect(next.title).toBe("");
  });
});

describe("inferBaselineResetKeys", () => {
  it("only flags keys that had a persisted baseline and diverged", () => {
    expect(
      inferBaselineResetKeys({ title: "hel", filter: "all" }, { filter: "all" }),
    ).toEqual(["title"]);
    expect(inferBaselineResetKeys({}, { title: "new-default" })).toEqual([]);
  });
});

describe("mergeDirtyReconcileResetKeys", () => {
  it("merges baseline inference with explicit Sync resetStateKeys", () => {
    const keys = mergeDirtyReconcileResetKeys(
      { a: 1, b: 2 },
      { a: 9, b: 2 },
      [" c ", "b"],
    );
    expect(keys.sort()).toEqual(["a", "b", "c"]);
  });

  it("does not treat never-persisted dirty as reset without explicit keys", () => {
    const resetKeys = mergeDirtyReconcileResetKeys(
      {},
      { title: "agent-default" },
      [],
    );
    const next = reconcileDirtyOverCanonical(
      { title: "agent-default" },
      { title: "user-typing" },
      resetKeys,
    );
    expect(next.title).toBe("user-typing");
  });
});

describe("resolvePendingInteractionIdempotencyKey", () => {
  it("reuses idempotency key for the same logical interaction", () => {
    const first = resolvePendingInteractionIdempotencyKey(
      null,
      "tool:submit::1",
      (k) => `idem-${k}-fixed`,
    );
    const second = resolvePendingInteractionIdempotencyKey(
      first.pending,
      "tool:submit::1",
      () => "should-not-run",
    );
    expect(second.idempotencyKey).toBe("idem-tool:submit::1-fixed");
    expect(second.pending).toBe(first.pending);
  });

  it("issues a new key when the logical interaction changes", () => {
    const first = resolvePendingInteractionIdempotencyKey(
      null,
      "tool:submit::1",
      (k) => `idem-${k}-a`,
    );
    const second = resolvePendingInteractionIdempotencyKey(
      first.pending,
      "tool:submit::2",
      (k) => `idem-${k}-b`,
    );
    expect(second.idempotencyKey).toBe("idem-tool:submit::2-b");
    expect(second.idempotencyKey).not.toBe(first.idempotencyKey);
  });
});

describe("runRendererMountPatchFlush", () => {
  it("skips flush when no canonical surface id", async () => {
    const flush = vi.fn(async () => undefined);
    const id = await runRendererMountPatchFlush(null, flush, "conv-1");
    expect(flush).not.toHaveBeenCalled();
    expect(id).toBeNull();
  });

  it("flushes deferred patches with renderer_mount args", async () => {
    const flush = vi.fn(async () => undefined);
    const id = await runRendererMountPatchFlush("surf-1", flush, "conv-9");
    expect(flush).toHaveBeenCalledWith({
      surfaceId: "surf-1",
      conversationId: "conv-9",
      sourceType: "renderer_mount",
    });
    expect(id).toMatch(/^rend-surf-1-/);
  });

  it("registers mount before flushing when registerMount is provided", async () => {
    const flush = vi.fn(async () => undefined);
    const register = vi.fn(async () => undefined);
    const id = await runRendererMountPatchFlush(
      "surf-2",
      flush,
      "conv-2",
      register,
      { definitionRevision: 3, stateRevision: 4, rendererInstanceId: "rend-fixed" },
    );
    expect(register).toHaveBeenCalledWith({
      surfaceId: "surf-2",
      rendererInstanceId: "rend-fixed",
      conversationId: "conv-2",
      definitionRevision: 3,
      stateRevision: 4,
    });
    expect(flush).toHaveBeenCalled();
    expect(id).toBe("rend-fixed");
  });

  it("skips flush when mount registration rejects", async () => {
    const flush = vi.fn(async () => undefined);
    const register = vi.fn(async () => {
      throw new Error("application identity mismatch");
    });
    await expect(
      runRendererMountPatchFlush("surf-3", flush, "conv-3", register, {
        rendererInstanceId: "rend-fail",
      }),
    ).rejects.toThrow(/application identity mismatch/);
    expect(register).toHaveBeenCalled();
    expect(flush).not.toHaveBeenCalled();
  });

  it("returns instance id when flush fails after successful register", async () => {
    const flush = vi.fn(async () => {
      throw new Error("flush boom");
    });
    const register = vi.fn(async () => undefined);
    const id = await runRendererMountPatchFlush(
      "surf-4",
      flush,
      "conv-4",
      register,
      { rendererInstanceId: "rend-keep" },
    );
    expect(id).toBe("rend-keep");
    expect(register).toHaveBeenCalled();
    expect(flush).toHaveBeenCalled();
  });
});

describe("startRendererMountLifecycle", () => {
  it("passes surfaceId into flushPatchScheduler", async () => {
    const flush = vi.fn(async () => undefined);
    const register = vi.fn(async () => undefined);
    const unregister = vi.fn(async () => undefined);
    const cleanup = startRendererMountLifecycle({
      surfaceId: "surf-life",
      conversationId: "conv-life",
      definitionRevision: 2,
      stateRevision: 1,
      registerMount: register,
      unregisterMount: unregister,
      flushPatchScheduler: flush,
    });
    await vi.waitFor(() => {
      expect(flush).toHaveBeenCalled();
    });
    expect(flush).toHaveBeenCalledWith({
      surfaceId: "surf-life",
      conversationId: "conv-life",
      sourceType: "renderer_mount",
    });
    expect(register).toHaveBeenCalledWith(
      expect.objectContaining({
        surfaceId: "surf-life",
        conversationId: "conv-life",
        definitionRevision: 2,
        stateRevision: 1,
      }),
    );
    cleanup();
  });

  it("cleanup unregisters with the instance id after register resolves", async () => {
    const flush = vi.fn(async () => undefined);
    const register = vi.fn(async () => undefined);
    const unregister = vi.fn(async () => undefined);
    const cleanup = startRendererMountLifecycle({
      surfaceId: "surf-clean",
      conversationId: "conv-clean",
      registerMount: register,
      unregisterMount: unregister,
      flushPatchScheduler: flush,
    });
    await vi.waitFor(() => {
      expect(register).toHaveBeenCalled();
      expect(flush).toHaveBeenCalled();
    });
    const calls = register.mock.calls as unknown as Array<
      [{ rendererInstanceId: string }]
    >;
    const registeredId = calls[0]?.[0]?.rendererInstanceId ?? "";
    expect(registeredId).toMatch(/^rend-surf-clean-/);
    // Let the lifecycle .then assign instanceId before cleanup.
    await Promise.resolve();
    cleanup();
    await vi.waitFor(() => {
      expect(unregister).toHaveBeenCalled();
    });
    expect(unregister).toHaveBeenCalledWith({
      surfaceId: "surf-clean",
      rendererInstanceId: registeredId,
    });
  });
});

describe("dirty state vs agent replace", () => {
  it("keeps unrelated dirty when agent replaces a previously persisted field", () => {
    const lastPersisted: Record<string, unknown> = {
      title: "hel",
      notes: "draft",
    };
    const canonical: Record<string, unknown> = {
      title: "Agent Title",
      notes: "draft",
      filter: "all",
    };
    const dirty = { title: "hello", notes: "user-notes" };
    expect(shouldPreserveComponent("replace", "text", "text")).toBe(false);
    const resetKeys = mergeDirtyReconcileResetKeys(lastPersisted, canonical, []);
    const next = reconcileDirtyOverCanonical(canonical, dirty, resetKeys);
    expect(next.title).toBe("Agent Title");
    expect(next.notes).toBe("user-notes");
    expect(next.filter).toBe("all");
  });
});

describe("scroll snapshot helpers", () => {
  it("captures and restores root scroll position", () => {
    const root = document.createElement("div");
    Object.defineProperty(root, "scrollHeight", { value: 400, configurable: true });
    Object.defineProperty(root, "clientHeight", { value: 100, configurable: true });
    root.scrollTop = 120;
    const snap = captureScrollSnapshot(root);
    root.scrollTop = 0;
    restoreScrollSnapshot(root, snap);
    expect(root.scrollTop).toBe(120);
  });
});

describe("route navigation no-op", () => {
  it("detects same route and params", () => {
    expect(
      isRouteNavigationNoOp("home", { tab: "overview" }, "home", {
        tab: "overview",
      }),
    ).toBe(true);
  });

  it("detects route or param changes", () => {
    expect(isRouteNavigationNoOp("home", {}, "settings", {})).toBe(false);
    expect(isRouteNavigationNoOp("home", { a: 1 }, "home", { a: 2 })).toBe(
      false,
    );
  });

  it("exposes history navigation guards", () => {
    expect(canNavigateBack(0)).toBe(false);
    expect(canNavigateBack(2)).toBe(true);
    expect(canNavigateForward(1, 3)).toBe(true);
    expect(canNavigateForward(2, 3)).toBe(false);
  });
});

describe("focus snapshot helpers", () => {
  it("restores focus to a component element that is itself a button", () => {
    const root = document.createElement("div");
    const button = document.createElement("button");
    button.dataset.componentId = "btn-primary";
    root.appendChild(button);
    document.body.appendChild(root);

    restoreFocusSnapshot(root, { componentId: "btn-primary" });
    expect(document.activeElement).toBe(button);

    document.body.removeChild(root);
  });

  it("restores focus to a nested input and restores selection range", () => {
    const root = document.createElement("div");
    const container = document.createElement("div");
    container.dataset.componentId = "card-1";
    const input = document.createElement("input");
    input.type = "text";
    input.value = "Hello world";
    container.appendChild(input);
    root.appendChild(container);
    document.body.appendChild(root);

    restoreFocusSnapshot(root, {
      componentId: "card-1",
      selectionStart: 2,
      selectionEnd: 5,
    });
    expect(document.activeElement).toBe(input);
    expect(input.selectionStart).toBe(2);
    expect(input.selectionEnd).toBe(5);

    document.body.removeChild(root);
  });

  it("restores focus by fieldId", () => {
    const root = document.createElement("div");
    const input = document.createElement("input");
    input.id = "search-input";
    root.appendChild(input);
    document.body.appendChild(root);

    restoreFocusSnapshot(root, { fieldId: "search-input" });
    expect(document.activeElement).toBe(input);

    document.body.removeChild(root);
  });

  it("captures active element snapshot within root", () => {
    const root = document.createElement("div");
    const container = document.createElement("div");
    container.dataset.componentId = "form-comp";
    const input = document.createElement("input");
    input.id = "my-field";
    input.value = "Testing";
    container.appendChild(input);
    root.appendChild(container);
    document.body.appendChild(root);

    input.focus();
    input.setSelectionRange(1, 4);

    const snap = captureFocusSnapshot(root);
    expect(snap.componentId).toBe("form-comp");
    expect(snap.fieldId).toBe("my-field");
    expect(snap.selectionStart).toBe(1);
    expect(snap.selectionEnd).toBe(4);

    document.body.removeChild(root);
  });

  it("captures and restores contenteditable selection offsets", () => {
    const root = document.createElement("div");
    const editable = document.createElement("div");
    editable.setAttribute("contenteditable", "true");
    editable.dataset.componentId = "rich-1";
    editable.id = "rich-field";
    editable.appendChild(document.createTextNode("Hello world"));
    root.appendChild(editable);
    document.body.appendChild(root);

    // Restore path (authoritative in production after focus lands).
    restoreFocusSnapshot(root, {
      componentId: "rich-1",
      fieldId: "rich-field",
      selectionAnchorOffset: 2,
      selectionFocusOffset: 7,
    });
    expect(document.activeElement).toBe(editable);
    const restored = window.getSelection();
    expect(restored?.rangeCount).toBeGreaterThan(0);
    expect(restored?.toString()).toBe("llo w");

    // Backward restore must still select the same span (direction is best-effort).
    restoreFocusSnapshot(root, {
      componentId: "rich-1",
      fieldId: "rich-field",
      selectionAnchorOffset: 7,
      selectionFocusOffset: 2,
    });
    expect(window.getSelection()?.toString()).toBe("llo w");

    // Capture path: pin activeElement (jsdom) and seed a live Selection.
    Object.defineProperty(document, "activeElement", {
      configurable: true,
      get: () => editable,
    });
    const textNode = editable.firstChild as Text;
    const range = document.createRange();
    range.setStart(textNode, 1);
    range.setEnd(textNode, 4);
    const sel = window.getSelection();
    sel?.removeAllRanges();
    sel?.addRange(range);

    const snap = captureFocusSnapshot(root);
    expect(snap.componentId).toBe("rich-1");
    expect(snap.fieldId).toBe("rich-field");
    // jsdom Selection support varies; offsets must be set when Selection is live.
    if (sel && sel.rangeCount > 0 && editable.contains(sel.anchorNode!)) {
      expect(snap.selectionAnchorOffset).toBe(1);
      expect(snap.selectionFocusOffset).toBe(4);
    }

    document.body.removeChild(root);
  });
});
