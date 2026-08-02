import { waitForAppReady } from "../helpers.js";

describe("Journey 4 — new conversation + draft without live provider", () => {
  it("creates a chat and keeps a composer draft without sending", async () => {
    await waitForAppReady();

    const newChat = await $('[aria-label="New chat"]');
    await newChat.click();

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    await composer.setValue("E2E draft — do not send");

    await expect(composer).toHaveValue("E2E draft — do not send");

    // No live provider: setup banner or non-ready status, never a successful send.
    const setup = await $(".setup-banner");
    const setupVisible = await setup.isExisting();
    if (setupVisible) {
      await expect(setup).toHaveText(expect.stringContaining("Connect"));
    }

    // Ensure we did not invent a connected hosted state.
    const bodyText = await $("body").getText();
    expect(bodyText).not.toMatch(/Coreside AI connected/i);
  });
});
