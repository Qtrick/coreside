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
  // Strip comments so assertions do not match prose (e.g. "inset:0" in notes).
  return (match?.[1] ?? "").replace(/\/\*[\s\S]*?\*\//g, "");
}

/**
 * Empty overlay hosts must not be full-viewport fixed layers. WebKit WebDriver
 * hit-testing can treat an empty inset:0 host as covering composer Apply even
 * with pointer-events:none (Journey 21). Expand only when a child is present.
 */
describe("window overlay host CSS contract", () => {
  const emptyHost = rule(
    ".app-shell > .window-overlay-root,\n.app-shell > .provider-modal-root,\nbody > .window-overlay-root,\n#coreside-window-overlay-root",
  );
  const populatedHost = rule(
    ".app-shell > .window-overlay-root:has(> *),\n.app-shell > .provider-modal-root:has(> *),\nbody > .window-overlay-root:has(> *),\n#coreside-window-overlay-root:has(> *)",
  );

  it("keeps an empty overlay host at zero size without covering the viewport", () => {
    expect(emptyHost).toMatch(/position:\s*fixed/);
    expect(emptyHost).toMatch(/width:\s*0/);
    expect(emptyHost).toMatch(/height:\s*0/);
    expect(emptyHost).toMatch(/pointer-events:\s*none/);
    // inset:auto defeats cascaded inset:0 (e.g. .provider-modal-root) so the
    // host does not remain over-constrained to the full viewport.
    expect(emptyHost).toMatch(/inset:\s*auto/);
    expect(emptyHost).not.toMatch(/inset:\s*0/);
    expect(emptyHost).not.toMatch(/width:\s*100%/);
    expect(emptyHost).not.toMatch(/height:\s*100%/);
  });

  it("expands the overlay host to the viewport only when it has a child", () => {
    expect(populatedHost).toMatch(/inset:\s*0/);
    expect(populatedHost).toMatch(/width:\s*auto/);
    expect(populatedHost).toMatch(/height:\s*auto/);
  });
});
