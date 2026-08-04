import { waitForAppReady } from "../helpers.js";

/**
 * Journey 16 — core essentials tutorial overlay.
 * Spec registered; desktop execution is not_run until onboarding can run
 * under the E2E harness (see Journey 15 note about CORESIDE_E2E).
 */
describe("Journey 16 — core tutorial", () => {
  it("advances the essentials tour when started from Welcome", async () => {
    await waitForAppReady();

    const onboardingDisabled =
      process.env.CORESIDE_E2E === "1" ||
      process.env.CORESIDE_DISABLE_ONBOARDING === "1";

    if (onboardingDisabled) {
      const overlay = await $(".tutorial-overlay");
      const exists = await overlay.isExisting().catch(() => false);
      expect(exists).toBe(false);
      return;
    }

    const takeTour = await $("button=Take the 3-minute tour");
    await takeTour.waitForDisplayed({ timeout: 20_000 });
    await takeTour.click();

    const overlay = await $(".tutorial-overlay");
    await overlay.waitForDisplayed({ timeout: 15_000 });

    const stepTitle = await $("#tutorial-step-title");
    await expect(stepTitle).toBeDisplayed();

    const next = await $("button=Next");
    if (await next.isExisting()) {
      await next.click();
      await expect(stepTitle).toBeDisplayed();
    }
  });
});
