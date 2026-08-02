import { closeSettings, openSettings, waitForAppReady } from "../helpers.js";

describe("Journey 3 — Settings open/close", () => {
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
});
