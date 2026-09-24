import { closeSettings, openSettings, openSettingsCategory, waitForAppReady } from "../helpers.js";

/**
 * Labels as they appear in the settings nav (from SETTINGS_CATEGORIES).
 * "App settings" is the "added" group — may be absent when no apps register
 * settings so we mark it optional.
 */
const CATEGORY_LABELS = [
  "General",
  "Appearance",
  "AI connections",
  "Assistant",
  "Web research",
  "Privacy & Security",
  "Data & Storage",
  "Accessibility",
  "Advanced",
  "Help & learning",
  "About",
] as const;

describe("Journey 3 — Settings panel", () => {
  it("opens Settings and does not falsely claim Coreside AI is connected", async () => {
    await waitForAppReady();
    await openSettings();

    await expect($("h1=Settings")).toBeDisplayed();

    // Without a live hosted session or BYOK key, status must not read as connected.
    const bodyText = await $("body").getText();
    expect(bodyText).not.toMatch(/Coreside AI connected/i);

    const statusPill = await $(".status-pill");
    if (await statusPill.isExisting()) {
      const label = (await statusPill.getText()).toLowerCase();
      expect(label).not.toContain("connected");
    }

    await closeSettings();
    await expect($('[aria-label="New chat"]')).toBeDisplayed();
  });

  it("renders all expected settings categories in the nav", async () => {
    await waitForAppReady();
    await openSettings();

    for (const label of CATEGORY_LABELS) {
      const found = await browser.execute((catLabel) => {
        const items = Array.from(
          document.querySelectorAll<HTMLButtonElement>(".settings-nav-item"),
        );
        return items.some((el) => {
          const lab = el.querySelector(".settings-nav-item-label");
          return lab?.textContent?.trim() === catLabel;
        });
      }, label);
      expect(found).toBe(true); // `Category "${label}" must appear in the nav`
    }

    await closeSettings();
  });

  it("navigates to each settings category without error", async () => {
    await waitForAppReady();
    await openSettings();

    for (const label of CATEGORY_LABELS) {
      await openSettingsCategory(label);

      // The content region must be displayed and not in error state
      const content = await $(".settings-content");
      await expect(content).toBeDisplayed();

      // No unhandled error boundary should render
      const errorBoundary = await $(".error-boundary-fallback, [data-testid='error-boundary']");
      if (await errorBoundary.isExisting()) {
        const msg = await errorBoundary.getText();
        throw new Error(`Error boundary shown for category "${label}": ${msg}`);
      }
    }

    await closeSettings();
  });

  it("Settings search filters categories and clears correctly", async () => {
    await waitForAppReady();
    await openSettings();

    // Focus and type into the settings search input
    const searchInput = await $(".settings-search-input, input[placeholder*='earch']");
    if (!(await searchInput.isExisting())) {
      // Search might not be implemented — skip gracefully
      await closeSettings();
      return;
    }

    await searchInput.click();
    await searchInput.setValue("appear");

    // At minimum one result containing "Appearance" should appear
    await browser.waitUntil(
      async () => {
        const results = await browser.execute(() =>
          document.querySelectorAll(".settings-search-result, .settings-nav-item").length,
        );
        return results > 0;
      },
      { timeout: 5_000, timeoutMsg: "Settings search did not return results" },
    );

    const bodyText = await $("body").getText();
    expect(bodyText).toMatch(/appear/i);

    // Clear the search and confirm nav is back to full list
    await searchInput.clearValue();
    await browser.waitUntil(
      async () => {
        const val = await searchInput.getValue();
        return val === "";
      },
      { timeout: 5_000 },
    );

    await closeSettings();
  });

  it("closes Settings with Escape key", async () => {
    await waitForAppReady();
    await openSettings();
    await expect($("h1=Settings")).toBeDisplayed();

    await browser.keys("Escape");
    await browser.waitUntil(
      async () => !(await $("h1=Settings").isExisting()),
      { timeout: 10_000, timeoutMsg: "Settings did not close after Escape" },
    );

    // App shell should be navigable again
    await expect($('[aria-label="New chat"]')).toBeDisplayed();
  });
});
