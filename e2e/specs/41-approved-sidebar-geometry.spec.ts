import { waitForAppReady } from "../helpers.js";

/**
 * Visual regression contract for the user-approved two-sidebar layout.
 * Exercises real WKWebView layout geometry, not only jsdom/class existence.
 */
describe("Journey 41 — approved dual-sidebar geometry", () => {
  it("keeps the entire contextual sidebar visible without crossing the workspace", async () => {
    await waitForAppReady();
    const measured = await browser.execute(() => {
      const rail = document.querySelector(".icon-rail");
      const contextual = document.querySelector(".contextual-sidebar");
      const shell = document.querySelector(".app-shell");
      const sidebar = document.querySelector(".dual-sidebar");
      const main = document.querySelector(".app-shell-main");
      if (!(rail && shell && sidebar && main)) return null;
      const r = rail.getBoundingClientRect();
      const s = sidebar.getBoundingClientRect();
      const m = main.getBoundingClientRect();
      const c = contextual?.getBoundingClientRect();
      const search = contextual?.querySelector(".contextual-search-box")?.getBoundingClientRect();
      const switcher = contextual?.querySelector(".contextual-segmented-switcher")?.getBoundingClientRect();
      return {
        railWidth: r.width,
        sidebarRight: s.right,
        mainLeft: m.left,
        contextualRight: c?.right ?? null,
        searchRight: search?.right ?? null,
        switcherRight: switcher?.right ?? null,
        viewport: window.innerWidth,
        hasContextual: !!contextual,
        duplicateProfiles: document.querySelectorAll(".icon-rail-avatar-btn").length,
      };
    });
    expect(measured).not.toBeNull();
    if (!measured) return;
    expect(measured.railWidth).toBeGreaterThan(55);
    expect(measured.duplicateProfiles).toBe(1);
    expect(Math.abs(measured.sidebarRight - measured.mainLeft)).toBeLessThanOrEqual(2);
    if (measured.hasContextual) {
      expect(measured.contextualRight).not.toBeNull();
      expect(Math.abs((measured.contextualRight ?? 0) - measured.mainLeft)).toBeLessThanOrEqual(2);
      expect((measured.searchRight ?? Infinity)).toBeLessThanOrEqual(measured.mainLeft + 1);
      expect((measured.switcherRight ?? Infinity)).toBeLessThanOrEqual(measured.mainLeft + 1);
    }
  });
});
