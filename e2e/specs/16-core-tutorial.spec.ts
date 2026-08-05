import { waitForAppReady } from "../helpers.js";

/**
 * Journey 16 — core essentials tutorial overlay.
 * Orchestrator runs with CORESIDE_E2E_ALLOW_ONBOARDING=1 on a clean profile.
 */
describe("Journey 16 — core tutorial", () => {
  it("advances the essentials tour when started from Welcome", async () => {
    await waitForAppReady();

    const onboardingDisabled =
      (process.env.CORESIDE_E2E === "1" &&
        process.env.CORESIDE_E2E_ALLOW_ONBOARDING !== "1") ||
      process.env.CORESIDE_DISABLE_ONBOARDING === "1";

    if (onboardingDisabled) {
      const overlay = await $(".tutorial-overlay");
      const exists = await overlay.isExisting().catch(() => false);
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
        timeoutMsg: "Welcome dialog never appeared before starting tour",
      },
    );

    const takeTour = await $("button=Take the 3-minute tour");
    await takeTour.waitForClickable({ timeout: 10_000 });
    await takeTour.click();

    await browser.waitUntil(
      async () => {
        const title = await $("#tutorial-step-title");
        if (!(await title.isExisting())) return false;
        const text = await title.getText();
        return text.length > 0;
      },
      {
        timeout: 20_000,
        timeoutMsg: "Tutorial overlay step title never appeared",
      },
    );

    const overlay = await $(".tutorial-overlay");
    await expect(overlay).toBeExisting();

    const stepTitle = await $("#tutorial-step-title");
    await expect(stepTitle).toBeExisting();
    const firstTitle = await stepTitle.getText();

    // WebDriver treats the popover as non-interactable while enter motion runs
    // (computed opacity can remain 0 briefly). Programmatic click matches user intent.
    await browser.execute(() => {
      (
        document.querySelector(
          ".tutorial-popover-actions .btn-primary",
        ) as HTMLButtonElement | null
      )?.click();
    });

    await browser.waitUntil(
      async () => {
        const title = await $("#tutorial-step-title");
        if (!(await title.isExisting())) return false;
        const text = await title.getText();
        return text.length > 0 && text !== firstTitle;
      },
      {
        timeout: 10_000,
        timeoutMsg: "Tutorial step did not advance after Next",
      },
    );
  });
});
