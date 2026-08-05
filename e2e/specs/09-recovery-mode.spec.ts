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

    const clickedEnter = await browser.execute(() => {
      const buttons = Array.from(document.querySelectorAll("button"));
      const btn = buttons.find(
        (el) => el.textContent?.trim() === "Enter Recovery Mode",
      ) as HTMLButtonElement | undefined;
      if (!btn || btn.disabled) return false;
      btn.click();
      return true;
    });
    expect(clickedEnter).toBe(true);

    await browser.waitUntil(
      async () => (await $("li*=Recovery Mode: On").isExisting()),
      {
        timeout: 15_000,
        timeoutMsg: "Recovery Mode did not turn on",
      },
    );

    const clickedExit = await browser.execute(() => {
      const buttons = Array.from(document.querySelectorAll("button"));
      const btn = buttons.find(
        (el) => el.textContent?.trim() === "Exit Recovery Mode",
      ) as HTMLButtonElement | undefined;
      if (!btn || btn.disabled) return false;
      btn.click();
      return true;
    });
    expect(clickedExit).toBe(true);

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
