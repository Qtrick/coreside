import {
  closeToolCanvas,
  denyPendingApprovalIfPresent,
  E2E_NOTE_INPUT_ID,
  E2E_TOOL_ID,
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

    // The shell owns horizontal scrolling. Generated content and the header
    // must reflow inside this pane rather than widening the desktop window.
    const layout = await browser.execute(() => {
      const root = document.documentElement;
      const main = document.querySelector<HTMLElement>(".app-shell-main");
      const canvas = document.querySelector<HTMLElement>(".tool-canvas");
      const body = document.querySelector<HTMLElement>(".tool-canvas-body");
      const close = document.querySelector<HTMLElement>(
        '[aria-label="Close tool canvas"], [aria-label="Back to chat"]',
      );
      return {
        rootOverflow: root.scrollWidth > root.clientWidth + 2,
        mainOverflow: Boolean(main && main.scrollWidth > main.clientWidth + 2),
        canvasOverflow: Boolean(
          canvas && canvas.scrollWidth > canvas.clientWidth + 2,
        ),
        bodyOverflow: Boolean(body && body.scrollWidth > body.clientWidth + 2),
        closeVisible: Boolean(
          close &&
            close.getBoundingClientRect().left >= 0 &&
            close.getBoundingClientRect().right <= window.innerWidth + 1,
        ),
      };
    });
    expect(layout).toMatchObject({
      rootOverflow: false,
      mainOverflow: false,
      canvasOverflow: false,
      bodyOverflow: false,
      closeVisible: true,
    });

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
