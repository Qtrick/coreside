import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { MANUAL_DOCK_ICON_SELECTION_ENABLED } from "@/lib/branding/manual-dock-icon-capability";
import { MANUAL_DOCK_ICON_LABELS } from "@/components/settings/ManualDockIconSelector";
import { SETTINGS_SEARCH_INDEX } from "@/lib/settings-categories";

describe("manual Dock icon product capability", () => {
  it("enables the product capability", () => {
    expect(MANUAL_DOCK_ICON_SELECTION_ENABLED).toBe(true);
  });

  it("retains selector labels in source", () => {
    expect(MANUAL_DOCK_ICON_LABELS).toEqual([
      "Classic Dark",
      "Classic Light",
      "Split",
    ]);
  });

  it("includes Dock icon in the active settings search index", () => {
    expect(SETTINGS_SEARCH_INDEX.some((e) => e.id === "dock-icon")).toBe(true);
  });

  it("does not render manual Dock selection from SettingsPanel source", () => {
    const src = readFileSync(
      resolve(process.cwd(), "src/components/settings/SettingsPanel.tsx"),
      "utf8",
    );
    expect(src).toContain("MANUAL_DOCK_ICON_SELECTION_ENABLED");
    expect(src).toContain("ManualDockIconSelector");
    // Active strings must not appear outside the gated mount path as free text
    // in the Appearance section — the dormant component holds them instead.
    expect(src.includes("Choose manually")).toBe(false);
    expect(src.includes("Classic Dark")).toBe(false);
    expect(src.includes("Manual choices update the running Dock tile")).toBe(
      false,
    );
  });

  it("keeps dormant UI implementation available for reactivation", () => {
    const src = readFileSync(
      resolve(
        process.cwd(),
        "src/components/settings/ManualDockIconSelector.tsx",
      ),
      "utf8",
    );
    expect(src).toContain("Classic Dark");
    expect(src).toContain("Classic Light");
    expect(src).toContain("Split");
    expect(src).toContain("Choose manually");
  });
});
