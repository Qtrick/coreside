import {
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

describe("Journey 10 — secondary tool window (partial: open + switch)", () => {
  it("opens the seeded tool in a secondary window and can switch WebDriver context", async () => {
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

    await switchTauriWindow("main");

    const windows = await listTauriWindows();
    expect(windows).toContain("main");
    expect(windows).toContain(label);
    // Partial: open + switch asserted. Close is not asserted — embedded WebDriver
    // cannot reliably invoke getCurrentWebviewWindow().close() from the guest.
  });
});
