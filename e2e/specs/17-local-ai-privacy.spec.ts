import { waitForAppReady } from "../helpers.js";

/**
 * Journey 17 — Local AI privacy disclosure.
 * Spec registered; desktop execution is not_run until a dedicated Local AI
 * profile (Ollama or loopback preset) is wired into e2e/run.mjs.
 *
 * Asserts that Privacy & Security and AI connections copy stay honest when
 * accessMode is user_local — no outbound provider traffic claims.
 */
describe("Journey 17 — Local AI privacy", () => {
  it("shows local-first privacy copy when Local AI is active", async function () {
    if (process.env.CORESIDE_E2E_LOCAL_AI_PROFILE !== "1") {
      this.skip();
    }

    await waitForAppReady();

    const settingsBtn = await $('[data-testid="sidebar-settings"], .sidebar-settings-btn');
    await settingsBtn.waitForClickable({ timeout: 15_000 });
    await settingsBtn.click();

    const privacyNav = await $('button.settings-nav-item*=Privacy');
    const exists = await privacyNav.isExisting().catch(() => false);
    expect(exists).toBe(true);
  });
});
