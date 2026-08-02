import {
  denyPendingApprovalIfPresent,
  E2E_GRANT_ID,
  openSettings,
  requireExistingSeed,
  scrollSettingsToHeading,
  waitForAppReady,
} from "../helpers.js";

describe("Journey 8 — grant revoke", () => {
  it("revokes the seeded remembered grant from Runtime permissions", async () => {
    requireExistingSeed("Journey 8");

    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openSettings();
    await scrollSettingsToHeading("Runtime permissions");

    const grantSelector = `[data-grant-id="${E2E_GRANT_ID}"]`;
    await browser.waitUntil(
      async () => browser.$(grantSelector).isExisting(),
      {
        timeout: 20_000,
        timeoutMsg: "Seeded remembered grant never appeared in Runtime permissions",
      },
    );

    // Click via DOM so WebDriver hit-testing cannot miss a scrolled settings row.
    const clicked = await browser.execute((grantId) => {
      const btn = document.querySelector(
        `[data-testid="revoke-grant-${grantId}"]`,
      ) as HTMLButtonElement | null;
      if (!btn || btn.disabled) return false;
      btn.click();
      return true;
    }, E2E_GRANT_ID);
    expect(clicked).toBe(true);

    await browser.waitUntil(
      async () => !(await browser.$(grantSelector).isExisting()),
      {
        timeout: 15_000,
        timeoutMsg: "Remembered grant still listed after revoke",
      },
    );
  });
});
