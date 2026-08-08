import { describe, expect, it } from "vitest";
import {
  effectiveDockIconForProduct,
  MANUAL_DOCK_ICON_SELECTION_ENABLED,
} from "@/lib/branding/manual-dock-icon-capability";
import {
  DEFAULT_DOCK_ICON,
  dockIconStatusLabel,
  parseDockIconConfig,
  validateDockIconConfig,
} from "@/types/agent";

describe("Dock icon config", () => {
  it("defaults to Follow macOS", () => {
    expect(DEFAULT_DOCK_ICON.authority).toBe("follow_macos");
    expect(dockIconStatusLabel(DEFAULT_DOCK_ICON)).toBe("Following macOS");
  });

  it("product gate forces Follow macOS while manual selection is dormant", () => {
    const staleManual = parseDockIconConfig({
      schemaVersion: 1,
      authority: "manual",
      artwork: "classic",
      style: "dark",
    });
    expect(staleManual.authority).toBe("manual");
    // While the product capability is off, effective authority must follow macOS
    // even if parse still understands dormant manual configs.
    expect(MANUAL_DOCK_ICON_SELECTION_ENABLED).toBe(false);
    expect(effectiveDockIconForProduct(staleManual)).toBe("follow_macos");
    expect(effectiveDockIconForProduct(DEFAULT_DOCK_ICON)).toBe("follow_macos");
  });

  it("migrates legacy auto/dark/light/split strings", () => {
    expect(parseDockIconConfig("auto").authority).toBe("follow_macos");
    expect(parseDockIconConfig("dark")).toEqual({
      schemaVersion: 1,
      authority: "manual",
      artwork: "classic",
      style: "dark",
    });
    expect(parseDockIconConfig("light").style).toBe("light");
    expect(parseDockIconConfig("split")).toEqual({
      schemaVersion: 1,
      authority: "manual",
      artwork: "split",
      style: "original",
    });
  });

  it("accepts versioned manual Split", () => {
    const split = parseDockIconConfig({
      schemaVersion: 1,
      authority: "manual",
      artwork: "split",
      style: "original",
    });
    expect(split.artwork).toBe("split");
    expect(dockIconStatusLabel(split)).toBe("Using Split");
  });

  it("fails closed on invalid values", () => {
    expect(parseDockIconConfig("tint").authority).toBe("follow_macos");
    expect(parseDockIconConfig({ schemaVersion: 9 }).authority).toBe(
      "follow_macos",
    );
  });

  it("Follow macOS strips artwork so no PNG preference remains", () => {
    expect(
      parseDockIconConfig({
        schemaVersion: 1,
        authority: "follow_macos",
        artwork: "split",
        style: "original",
      }),
    ).toEqual(DEFAULT_DOCK_ICON);
    expect(DEFAULT_DOCK_ICON.artwork).toBeUndefined();
    expect(DEFAULT_DOCK_ICON.style).toBeUndefined();
  });

  it("normalizes follow extras and split style", () => {
    expect(
      parseDockIconConfig({
        schemaVersion: 1,
        authority: "follow_macos",
        artwork: "classic",
        style: "dark",
      }),
    ).toEqual(DEFAULT_DOCK_ICON);
    expect(
      parseDockIconConfig({
        schemaVersion: 1,
        authority: "manual",
        artwork: "split",
        style: "dark",
      }),
    ).toEqual({
      schemaVersion: 1,
      authority: "manual",
      artwork: "split",
      style: "original",
    });
  });

  it("validate rejects invalid manual combos (commit contract)", () => {
    expect(() =>
      validateDockIconConfig({
        schemaVersion: 1,
        authority: "manual",
        artwork: "split",
        style: "dark",
      }),
    ).toThrow(/original/i);
    expect(() =>
      validateDockIconConfig({
        schemaVersion: 1,
        authority: "manual",
        artwork: "classic",
        style: "original",
      }),
    ).toThrow(/dark or light/i);
    expect(
      validateDockIconConfig({
        schemaVersion: 1,
        authority: "follow_macos",
        artwork: "split",
        style: "original",
      }),
    ).toEqual(DEFAULT_DOCK_ICON);
  });
});
