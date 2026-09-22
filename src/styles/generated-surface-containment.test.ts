import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const css = fs.readFileSync(path.join(root, "src/styles/global.css"), "utf8");

function rule(selectors: string): string {
  const escaped = selectors.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = css.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`));
  expect(match, `Missing CSS rule for ${selectors}`).not.toBeNull();
  return match?.[1] ?? "";
}

/**
 * Generated tools can compose arbitrary trusted primitives. These CSS contracts
 * keep their min-content size inside the Tool Canvas instead of requiring every
 * future tool definition to know the host pane width.
 */
describe("generated surface containment", () => {
  it("keeps the canvas body as the explicit, bounded scroll owner", () => {
    const body = rule(".tool-canvas-body");
    expect(body).toMatch(/min-width:\s*0/);
    expect(body).toMatch(/min-height:\s*0/);
    expect(body).toMatch(/max-width:\s*100%/);
    expect(body).toMatch(/overflow-y:\s*auto/);
    expect(body).toMatch(/overflow-x:\s*hidden/);
  });

  it("makes generated layout primitives shrink or reflow before they widen the host", () => {
    const columns = rule(".tr-container,\n.tr-column");
    expect(columns).toMatch(/width:\s*100%/);
    expect(columns).toMatch(/min-width:\s*0/);
    expect(columns).toMatch(/max-width:\s*100%/);
    expect(columns).toMatch(/overflow-wrap:\s*anywhere/);

    const rows = rule(".tr-row,\n.tr-button-group");
    expect(rows).toMatch(/flex-wrap:\s*wrap/);
    expect(rows).toMatch(/min-width:\s*0/);
    expect(rows).toMatch(/max-width:\s*100%/);

    const descendants = rule(".tr-container > *,\n.tr-column > *,\n.tr-row > *,\n.tr-card > *,\n.tr-button-group > *");
    expect(descendants).toMatch(/min-width:\s*0/);
    expect(descendants).toMatch(/max-width:\s*100%/);
  });

  it("allows generated fields to reflow instead of imposing a fixed minimum width", () => {
    const field = rule(".tr-field");
    expect(field).toMatch(/min-width:\s*0/);
    expect(field).toMatch(/flex:\s*1\s+1\s+12rem/);
    expect(field).toMatch(/max-width:\s*100%/);
    expect(field).not.toMatch(/min-width:\s*160px/);
  });

  it("wraps the wallpaper category tabs instead of widening the settings panel", () => {
    // Root cause of the settings horizontal scrollbar: a 10-tab flex row with
    // no wrap/shrink guards inside .settings-panel (overflow: auto).
    const tabs = rule(".wallpaper-category-tabs");
    expect(tabs).toMatch(/display:\s*flex/);
    expect(tabs).toMatch(/flex-wrap:\s*wrap/);
    expect(tabs).toMatch(/min-width:\s*0/);
    expect(tabs).toMatch(/max-width:\s*100%/);

    const tab = rule(".wallpaper-category-tab");
    expect(tab).toMatch(/min-width:\s*0/);
    expect(tab).toMatch(/max-width:\s*100%/);
  });

  it("lets wallpaper tuning and hero rows wrap on narrow settings widths", () => {
    const tuning = rule(".wallpaper-tuning-panel");
    expect(tuning).toMatch(/flex-wrap:\s*wrap/);
    expect(tuning).toMatch(/min-width:\s*0/);
    expect(tuning).toMatch(/max-width:\s*100%/);

    const hero = rule(".wallpaper-hero-overlay");
    expect(hero).toMatch(/flex-wrap:\s*wrap/);
  });

  it("scales generated images to the surface instead of clipping their intrinsic width", () => {
    const image = rule(".tr-image img");
    expect(image).toMatch(/display:\s*block/);
    expect(image).toMatch(/max-inline-size:\s*100%/);
    expect(image).toMatch(/height:\s*auto/);
  });
});
