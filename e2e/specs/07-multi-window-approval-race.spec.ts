import {
  E2E_TOOL_ID,
  listTauriWindows,
  openPersonalTool,
  openToolInWindow,
  requireExistingSeed,
  switchTauriWindow,
  waitForAppReady,
  waitForPendingApproval,
  waitForToolCanvas,
} from "../helpers.js";

describe("Journey 7 — multi-window approval race (partial)", () => {
  it("keeps the pending approval on main while a secondary tool window opens", async () => {
    requireExistingSeed("Journey 7");

    await waitForAppReady();
    await waitForPendingApproval();

    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);
    await openToolInWindow();

    await browser.waitUntil(
      async () => (await listTauriWindows()).includes(`tool-${E2E_TOOL_ID}`),
      {
        timeout: 20_000,
        timeoutMsg: "Secondary tool window label never appeared",
      },
    );

    // Main window should still host the approval modal.
    await switchTauriWindow("main");
    await waitForPendingApproval();

    const windows = await listTauriWindows();
    expect(windows).toContain("main");
    expect(windows).toContain(`tool-${E2E_TOOL_ID}`);

    // Partial coverage: WebDriver session stays on main; we do not assert
    // duplicate approval UI inside the secondary window in this harness yet.
  });
});
