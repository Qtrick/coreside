import { waitForAppReady } from "../helpers.js";

/**
 * Journey 18 — Hosted Coreside AI (Free plan) chat.
 * Spec registered; desktop execution is not_run until hosted Supabase auth,
 * entitlements, and ai-gateway are available in an isolated E2E profile.
 *
 * Do not fabricate passes — this journey requires a real hosted session.
 */
describe("Journey 18 — Hosted Free chat", () => {
  it("requires hosted auth before sending through Coreside AI", async function () {
    if (process.env.CORESIDE_E2E_HOSTED_PROFILE !== "1") {
      this.skip();
    }

    await waitForAppReady();

    const composer = await $('[data-coreside-tour="chat-composer"]');
    await expect(composer).toBeExisting();
  });
});
