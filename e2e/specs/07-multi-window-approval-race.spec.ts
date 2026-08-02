import {
  approveOnce,
  E2E_APPROVAL_ID,
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

describe("Journey 7 — multi-window approval exact-once", () => {
  it("shows one approval, propagates the decision, and executes once", async () => {
    requireExistingSeed("Journey 7");

    await waitForAppReady();
    await waitForPendingApproval();

    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);
    await openToolInWindow();

    const toolLabel = `tool-${E2E_TOOL_ID}`;
    await browser.waitUntil(
      async () => (await listTauriWindows()).includes(toolLabel),
      {
        timeout: 20_000,
        timeoutMsg: "Secondary tool window label never appeared",
      },
    );

    await switchTauriWindow("main");
    await waitForPendingApproval();

    // Secondary window must not host a second approval card.
    await switchTauriWindow(toolLabel);
    const secondaryHasApproval = await browser.execute((id) => {
      return Boolean(document.querySelector(`#approval-${id}-title`));
    }, E2E_APPROVAL_ID);
    expect(secondaryHasApproval).toBe(false);

    await switchTauriWindow("main");
    const mainCountBefore = await browser.execute((id) => {
      return document.querySelectorAll(`#approval-${id}-title`).length;
    }, E2E_APPROVAL_ID);
    expect(mainCountBefore).toBe(1);

    await approveOnce();

    await browser.waitUntil(
      async () => {
        return browser.execute((id) => {
          return !document.querySelector(`#approval-${id}-title`);
        }, E2E_APPROVAL_ID);
      },
      {
        timeout: 15_000,
        timeoutMsg: "Approval remained after approve-once",
      },
    );

    await switchTauriWindow(toolLabel);
    const secondaryStillClean = await browser.execute((id) => {
      return !document.querySelector(`#approval-${id}-title`);
    }, E2E_APPROVAL_ID);
    expect(secondaryStillClean).toBe(true);

    await switchTauriWindow("main");
    const windows = await listTauriWindows();
    expect(windows).toContain("main");
    expect(windows).toContain(toolLabel);
  });
});
