import {
  closeToolCanvas,
  denyPendingApprovalIfPresent,
  E2E_TOOL_ID,
  listTauriWindows,
  openPersonalTool,
  openToolInWindow,
  requireExistingSeed,
  switchTauriWindow,
  waitForAppReady,
  waitForSeededTool,
  waitForToolCanvas,
} from "../helpers.js";

describe("Journey 10 — secondary tool window lifecycle", () => {
  it("opens, focuses, restores state, and keeps main stable after close path", async () => {
    requireExistingSeed("Journey 10");

    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await waitForSeededTool();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);
    await openToolInWindow();

    const label = `tool-${E2E_TOOL_ID}`;
    await browser.waitUntil(async () => (await listTauriWindows()).includes(label), {
      timeout: 20_000,
      timeoutMsg: `Secondary window ${label} never appeared`,
    });

    await switchTauriWindow(label);

    await browser.waitUntil(
      async () => {
        return browser.execute((toolId) => {
          return Boolean(
            document.querySelector(`[data-tool-id="${toolId}"]`) ||
              document.querySelector(`.tool-canvas-body[data-tool-id="${toolId}"]`),
          );
        }, E2E_TOOL_ID);
      },
      {
        timeout: 15_000,
        timeoutMsg: "Tool UI did not render in secondary window",
      },
    );

    // Close via main-window canvas close (authoritative UI path). Secondary
    // window may remain until OS close; assert main remains stable.
    await switchTauriWindow("main");
    await closeToolCanvas();

    await browser.waitUntil(
      async () => !(await $(`.tool-canvas-body[data-tool-id="${E2E_TOOL_ID}"]`).isExisting()),
      {
        timeout: 10_000,
        timeoutMsg: "Main tool canvas did not close",
      },
    );

    const sidebar = await $('[aria-label="New chat"]');
    await expect(sidebar).toExist();

    // Re-open canvas and secondary window path again.
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);
    await openToolInWindow();
    await browser.waitUntil(async () => (await listTauriWindows()).includes(label), {
      timeout: 20_000,
      timeoutMsg: `Secondary window ${label} did not reappear`,
    });

    const windows = await listTauriWindows();
    expect(windows).toContain("main");
    expect(windows).toContain(label);
  });
});
