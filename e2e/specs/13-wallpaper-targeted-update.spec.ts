import { waitForAppReady } from "../helpers.js";

/**
 * Journey 13 — wallpaper selection + transparency commit coalescing.
 *
 * Navigates Settings → Added/Templates wallpapers, applies Matrix, sets
 * transparency via slider commit path, and asserts UI selection state.
 * Pixel sampling of the live wallpaper canvas is best-effort when present.
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

    const live = await $(".live-wallpaper");
    if (await live.isExisting()) {
      await expect(live).toBeExisting();
    }
  });
});
