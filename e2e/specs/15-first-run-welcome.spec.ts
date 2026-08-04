import { waitForAppReady } from "../helpers.js";

/**
 * Journey 15 — first-run welcome.
 * Spec registered; desktop execution is not_run until the suite is wired
 * into e2e/run.mjs with a clean profile that does NOT set CORESIDE_E2E in a
 * way that disables onboarding (or uses an explicit enable path).
 *
 * Current E2E harness sets CORESIDE_E2E=1 which disables onboarding via
 * onboarding_disabled_from_env — so this journey cannot pass under the
 * default harness without a dedicated env/profile strategy.
 */
describe("Journey 15 — first-run welcome", () => {
  it("shows Welcome on a clean profile when onboarding is enabled", async () => {
    await waitForAppReady();

    // When CORESIDE_E2E disables onboarding, assert absence instead of inventing a pass.
    const onboardingDisabled =
      process.env.CORESIDE_E2E === "1" ||
      process.env.CORESIDE_DISABLE_ONBOARDING === "1";

    if (onboardingDisabled) {
      const welcome = await $("#onboarding-welcome-title");
      const exists = await welcome.isExisting().catch(() => false);
      expect(exists).toBe(false);
      return;
    }

    const title = await $("#onboarding-welcome-title");
    await title.waitForDisplayed({ timeout: 20_000 });
    await expect(title).toHaveText(expect.stringContaining("Welcome to Coreside"));

    const takeTour = await $("button=Take the 3-minute tour");
    await expect(takeTour).toBeDisplayed();
    const explore = await $("button=Explore on my own");
    await expect(explore).toBeDisplayed();
  });
});
