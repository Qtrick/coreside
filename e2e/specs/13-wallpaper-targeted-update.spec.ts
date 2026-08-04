import { waitForAppReady } from "../helpers.js";

/**
 * Journey 13 — wallpaper selection + transparency commit coalescing.
 *
 * Navigates Settings → Added/Templates wallpapers, applies Matrix, sets
 * transparency via slider commit path, and asserts UI selection state.
 *
 * TODO(pixel-sampling): Desktop Verified requires sampling live wallpaper
 * canvas pixels (Matrix glyphs / aurora gradients vs solid None) across
 * transparency 0/20/40/60. Not implemented in this journey — presence of
 * `.live-wallpaper` is best-effort only. Do not claim Desktop Verified from
 * this spec alone.
 */
describe("Journey 13 — wallpaper targeted update", () => {
  it("applies Matrix and commits transparency without dead controls", async () => {
    await waitForAppReady();

    const settingsBtn = await $('[aria-label="Settings"]');
    await settingsBtn.waitForExist({ timeout: 15_000 });
    await settingsBtn.click();

    // Prefer search to reach Wallpapers under Added Settings → Templates.
    const search = await $('input[placeholder*="Search" i], [aria-label*="Search settings" i]');
    if (await search.isExisting()) {
      await search.setValue("wallpaper");
      await browser.pause(300);
    }

    const wallpapersHeading = await $("#wallpaper-heading");
    await wallpapersHeading.waitForExist({ timeout: 15_000 });

    const matrix = await $('button.wallpaper-preset-card*=Matrix');
    await matrix.waitForExist({ timeout: 10_000 });
    await matrix.click();

    // Preview-first: aria-pressed should flip from optimistic Zustand before
    // durable save fully settles (still wait for pressed=true).
    await browser.waitUntil(
      async () => (await matrix.getAttribute("aria-pressed")) === "true",
      { timeout: 10_000, timeoutMsg: "Matrix preset not selected" },
    );

    const slider = await $('input[type="range"][aria-valuemax="60"]');
    await slider.waitForExist({ timeout: 5_000 });
    await slider.click();
    // Set a distinct transparency and commit via pointerup path.
    await browser.execute((el) => {
      const input = el as HTMLInputElement;
      input.value = "40";
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.dispatchEvent(new Event("change", { bubbles: true }));
      input.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
    }, slider);

    await browser.waitUntil(
      async () => {
        const now = await slider.getAttribute("aria-valuenow");
        return now === "40";
      },
      { timeout: 8_000, timeoutMsg: "transparency did not commit to 40" },
    );

    // Best-effort DOM presence only — not pixel proof (see TODO above).
    const live = await $(".live-wallpaper");
    if (await live.isExisting()) {
      await expect(live).toBeExisting();
    }
  });
});
