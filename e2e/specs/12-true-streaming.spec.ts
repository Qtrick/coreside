import { waitForAppReady } from "../helpers.js";

/**
 * Journey 12 — true provider streaming (mock live stream probe).
 *
 * Requires AI_PROVIDER=mock (default in e2e/wdio.conf.ts).
 * The mock adapter emits TextDelta before delayed completion when the
 * user message contains "live stream probe".
 */
describe("Journey 12 — true provider streaming", () => {
  it("renders a live assistant delta before the turn completes", async () => {
    await waitForAppReady();

    const newChat = await $('[aria-label="New chat"]');
    await newChat.click();

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    await composer.setValue("Please run a live stream probe now");

    // Prefer explicit Send control; Composer uses aria-label="Send message".
    const send = await $('[aria-label="Send message"]');
    await send.waitForExist({ timeout: 10_000 });
    await send.waitForClickable({ timeout: 5_000 });
    await send.click();

    const stream = await $('[data-testid="assistant-stream-text"]');
    await stream.waitForExist({ timeout: 10_000 });
    const earlyText = await stream.getText();
    expect(earlyText.toLowerCase()).toContain("live stream");

    // Final assistant message after turn completes.
    await browser.waitUntil(
      async () => {
        const body = await $("body").getText();
        return /live stream probe complete/i.test(body);
      },
      {
        timeout: 20_000,
        timeoutMsg: "expected final live stream probe message",
      },
    );
  });
});
