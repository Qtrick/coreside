import {
  closeSettings,
  denyPendingApprovalIfPresent,
  openSettings,
  requireExistingSeed,
  scrollSettingsToHeading,
  waitForAppReady,
} from "../helpers.js";

describe("Journey 9 — Recovery Mode enter/exit", () => {
  it("enters and exits Recovery Mode from Settings → Recovery", async () => {
    requireExistingSeed("Journey 9");

    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openSettings();
    await scrollSettingsToHeading("Recovery");

    const enter = await $("button=Enter Recovery Mode");
    await enter.waitForClickable({ timeout: 10_000 });
    await enter.click();

    await browser.waitUntil(
      async () => (await $("li*=Recovery Mode: On").isExisting()),
      {
        timeout: 15_000,
        timeoutMsg: "Recovery Mode did not turn on",
      },
    );

    const exit = await $("button=Exit Recovery Mode");
    await exit.waitForClickable({ timeout: 10_000 });
    await exit.click();

    await browser.waitUntil(
      async () => (await $("li*=Recovery Mode: Off").isExisting()),
      {
        timeout: 15_000,
        timeoutMsg: "Recovery Mode did not turn off",
      },
    );

    await closeSettings();
  });
});
