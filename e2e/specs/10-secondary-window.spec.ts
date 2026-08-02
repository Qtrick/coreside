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
  it("opens, closes the native secondary window, and reopens cleanly", async () => {
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

    // Close the native secondary window (not merely the main Tool Canvas).
    await browser.closeWindow();
    await switchTauriWindow("main");

    await browser.waitUntil(
      async () => !(await listTauriWindows()).includes(label),
      {
        timeout: 20_000,
        timeoutMsg: `Secondary window ${label} remained after closeWindow`,
      },
    );

    const sidebar = await $('[aria-label="New chat"]');
    await expect(sidebar).toExist();

    // Re-open secondary window and confirm a single label returns.
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);
    await openToolInWindow();
    await browser.waitUntil(async () => (await listTauriWindows()).includes(label), {
      timeout: 20_000,
      timeoutMsg: `Secondary window ${label} did not reappear`,
    });

    const windows = await listTauriWindows();
    expect(windows.filter((w) => w === label).length).toBe(1);
    expect(windows).toContain("main");

    await closeToolCanvas();
  });
});
