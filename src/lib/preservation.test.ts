import { describe, expect, it } from "vitest";
import {
  shouldPreserveComponent,
  captureScrollSnapshot,
  restoreScrollSnapshot,
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
