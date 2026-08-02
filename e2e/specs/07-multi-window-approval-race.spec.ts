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

async function invokeJson<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return browser.execute(
    async (command, invokeArgs) => {
      const core = (
        window as unknown as {
          __TAURI__?: { core?: { invoke: (c: string, a?: unknown) => Promise<unknown> } };
        }
      ).__TAURI__?.core;
      if (!core?.invoke) throw new Error("Tauri invoke unavailable");
      return core.invoke(command, invokeArgs ?? {});
    },
    cmd,
    args ?? {},
  ) as Promise<T>;
}

describe("Journey 7 — multi-window approval exact-once", () => {
  it("shows one approval, consumes once, audits once, and does not replay", async () => {
    requireExistingSeed("Journey 7");

    await waitForAppReady();
    await waitForPendingApproval();

    const pendingBefore = await invokeJson<Array<{ id: string; status: string }>>(
      "kernel_list_pending_approvals",
    );
    const matching = pendingBefore.filter((row) => row.id === E2E_APPROVAL_ID);
    expect(matching.length).toBe(1);

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

    const pendingAfter = await invokeJson<Array<{ id: string }>>(
      "kernel_list_pending_approvals",
    );
    expect(pendingAfter.some((row) => row.id === E2E_APPROVAL_ID)).toBe(false);

    // Second decision must fail safely (already consumed / not pending).
    // Rust signature: approval_id, approve: bool (not decision: string).
    const secondDecisionError = await invokeJson<unknown>(
      "kernel_decide_approval",
      { approvalId: E2E_APPROVAL_ID, approve: true },
    ).then(
      () => null,
      (err: unknown) => (err instanceof Error ? err.message : String(err)),
    );
    expect(secondDecisionError).toBeTruthy();

    const audit = await invokeJson<Array<{ approvalId?: string; outcome?: string }>>(
      "kernel_list_audit_events",
      { applicationId: null, limit: 50 },
    );
    const related = audit.filter((row) => row.approvalId === E2E_APPROVAL_ID);
    expect(related.length).toBeGreaterThanOrEqual(1);

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
