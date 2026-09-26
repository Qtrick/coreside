import { describe, expect, it } from "vitest";
import {
  shouldPreserveComponent,
  captureScrollSnapshot,
  restoreScrollSnapshot,
  captureFocusSnapshot,
  restoreFocusSnapshot,
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
});
