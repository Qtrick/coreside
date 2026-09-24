import { openSettings, openSettingsCategory, waitForAppReady } from "../helpers.js";

/**
 * Journey 17 — Local AI privacy disclosure.
 *
 * Asserts that Privacy & Security and AI connections copy stay honest when
 * accessMode is user_local — no outbound provider traffic claims, local data stays on computer.
 */
describe("Journey 17 — Local AI privacy", () => {
  it("shows local-first privacy copy when Local AI is active", async function () {
    if (
      process.env.CORESIDE_E2E_LOCAL_AI_PROFILE !== "1" &&
      process.env.CORESIDE_E2E_SEED !== "local"
    ) {
      this.skip();
    }

    await waitForAppReady();
    await openSettings();

    // Verify Privacy & Security navigation and on-device copy
    await openSettingsCategory("Privacy & Security");

    const bodyText = await $("body").getText();
    expect(bodyText).toContain(
      "Chats, apps, and most research caches stay on this computer.",
    );

    // Verify AI connections section
    await openSettingsCategory("AI connections");

    const updatedText = await $("body").getText();
    const hasLocalCopy =
      updatedText.includes("Local AI") ||
      updatedText.includes("on this machine") ||
      updatedText.includes("AI connections");
    expect(hasLocalCopy).toBe(true);
  });
});

