import { waitForAppReady } from "../helpers.js";

/**
 * Journey 15 — first-run welcome.
 * Orchestrator runs with CORESIDE_E2E_ALLOW_ONBOARDING=1 on a clean profile.
 */
describe("Journey 15 — first-run welcome", () => {
  it("shows Welcome on a clean profile when onboarding is enabled", async () => {
    await waitForAppReady();

    // When CORESIDE_E2E disables onboarding, assert absence instead of inventing a pass.
    const onboardingDisabled =
      (process.env.CORESIDE_E2E === "1" &&
        process.env.CORESIDE_E2E_ALLOW_ONBOARDING !== "1") ||
      process.env.CORESIDE_DISABLE_ONBOARDING === "1";

    if (onboardingDisabled) {
      const welcome = await $("#onboarding-welcome-title");
      const exists = await welcome.isExisting().catch(() => false);
      expect(exists).toBe(false);
      return;
    }

    await browser.waitUntil(
      async () => {
        const title = await $("#onboarding-welcome-title");
        return title.isDisplayed().catch(() => false);
      },
      {
        timeout: 25_000,
        timeoutMsg: "Welcome dialog never appeared on onboarding-enabled clean profile",
      },
    );

    const title = await $("#onboarding-welcome-title");
    await expect(title).toHaveText(expect.stringContaining("Welcome to Coreside"));

    const takeTour = await $("button=Take the 3-minute tour");
    await expect(takeTour).toBeDisplayed();
    const explore = await $('button[data-action="dismiss-welcome"], button*=Explore on my own');
    await expect(explore).toBeDisplayed();
    const continueBtn = await $('button[data-action="continue-onboarding"]');
    await expect(continueBtn).toBeDisplayed();

    // Advance to Screen 2: Appearance
    await continueBtn.click();

    await browser.waitUntil(
      async () => {
        const title = await $("#onboarding-appearance-title");
        return title.isDisplayed().catch(() => false);
      },
      {
        timeout: 10_000,
        timeoutMsg: "Appearance selection step never appeared after clicking Continue",
      },
    );

    const appearanceTitle = await $("#onboarding-appearance-title");
    await expect(appearanceTitle).toHaveText(expect.stringContaining("Choose your appearance"));

    // Verify theme preview options
    const lightOption = await $('[data-testid="theme-option-light"]');
    await expect(lightOption).toBeDisplayed();
    const darkOption = await $('[data-testid="theme-option-dark"]');
    await expect(darkOption).toBeDisplayed();

    // Select dark theme in draft
    await darkOption.click();

    // Confirm appearance and enter application
    const startBtn = await $('button[data-action="start-using-coreside"]');
    await expect(startBtn).toBeDisplayed();
    await startBtn.click();

    // Verify modal dismissed and long tour does NOT auto-launch
    await browser.waitUntil(
      async () => {
        const modal = await $(".onboarding-modal");
        const modalExists = await modal.isDisplayed().catch(() => false);
        return !modalExists;
      },
      {
        timeout: 10_000,
        timeoutMsg: "Onboarding dialog remained visible after Start using Coreside",
      },
    );

    const tourOverlay = await $(".tutorial-overlay");
    const tourExists = await tourOverlay.isDisplayed().catch(() => false);
    expect(tourExists).toBe(false);
  });
});
