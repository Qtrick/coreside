import {
  assertNoLayoutOverflow,
  closeToolCanvas,
  denyPendingApprovalIfPresent,
  E2E_NOTE_INPUT_ID,
  E2E_TOOL_ID,
  measureToolCanvasLayout,
  openPersonalTool,
  requireExistingSeed,
  waitForAppReady,
  waitForSeededTool,
  waitForToolCanvas,
} from "../helpers.js";

describe("Journey 5 — generated tool open + state persist", () => {
  it("opens the seeded tool and persists text input state across close/reopen", async () => {
    requireExistingSeed("Journey 5");

    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await waitForSeededTool();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

    const layout = await measureToolCanvasLayout();
    assertNoLayoutOverflow(layout, "initial");

    const canResize =
      typeof browser.setWindowRect === "function" &&
      typeof browser.getWindowRect === "function";
    if (canResize) {
      const original = await browser.getWindowRect();
      const dpr = await browser.execute(() => window.devicePixelRatio || 1);
      try {
        for (const size of [
          { width: 1024, height: 720 },
          { width: 1100, height: 720 },
          { width: 1200, height: 800 },
          { width: 1280, height: 800 },
          { width: 1440, height: 900 },
          { width: 900, height: 700 },
        ]) {
          await browser.setWindowRect(
            original.x,
            original.y,
            Math.round(size.width * dpr),
            Math.round(size.height * dpr),
          );
          await browser.waitUntil(
            async () => {
              const w = await browser.execute(() => window.innerWidth);
              return Math.abs(w - size.width) <= 2;
            },
            {
              timeout: 5_000,
              timeoutMsg: `window.innerWidth did not settle at ${size.width}`,
            },
          );
          const resized = await measureToolCanvasLayout();
          assertNoLayoutOverflow(resized, `${size.width}x${size.height}`);
        }
      } finally {
        await browser.setWindowRect(
          original.x,
          original.y,
          original.width,
          original.height,
        );
      }
    }

    const input = await $(`#${E2E_NOTE_INPUT_ID}`);
    await input.waitForExist({ timeout: 10_000 });
    await input.setValue("E2E persisted note");

    await closeToolCanvas();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

    const reopened = await $(`#${E2E_NOTE_INPUT_ID}`);
    await expect(reopened).toHaveValue("E2E persisted note");
  });
});
