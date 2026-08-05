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
    const explore = await $("button=Explore on my own");
    await expect(explore).toBeDisplayed();
  });
});
