import {
  approveOnce,
  E2E_APPROVAL_ID,
  E2E_TOOL_ID,
  invokeFromCurrentWindow,
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
  it("shows one approval, consumes once, audits once, and does not replay", async () => {
    requireExistingSeed("Journey 7");

    await waitForAppReady();
    // Prefer IPC over DOM — catches race where the card was auto-dismissed.
    await browser.waitUntil(
      async () => {
        const outcome = await invokeFromCurrentWindow(
          "kernel_list_pending_approvals",
        );
        if (!outcome.ok) return false;
        const rows = outcome.result as Array<{ id: string; status: string }>;
        return rows.some((row) => row.id === E2E_APPROVAL_ID && row.status === "pending");
      },
      {
        timeout: 20_000,
        timeoutMsg: "Seeded pending approval never listed by kernel",
      },
    );
    await waitForPendingApproval();

    const pendingBefore = await invokeFromCurrentWindow(
      "kernel_list_pending_approvals",
    );
    expect(pendingBefore.ok).toBe(true);
    const matching = (
      pendingBefore.ok ? (pendingBefore.result as Array<{ id: string; status: string }>) : []
    ).filter((row) => row.id === E2E_APPROVAL_ID);
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
    // Wait until the secondary webview is interactive — executing too early
    // against a mid-bootstrap tool window can hang the WDIO channel.
    await browser.waitUntil(
      async () => {
        try {
          return await browser.execute(() =>
            Boolean(document.querySelector(".tool-window")),
          );
        } catch {
          return false;
        }
      },
      {
        timeout: 20_000,
        timeoutMsg: "Secondary tool window never became interactive",
      },
    );
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

    const pendingAfter = await invokeFromCurrentWindow(
      "kernel_list_pending_approvals",
    );
    expect(pendingAfter.ok).toBe(true);
    const stillPending = (
      pendingAfter.ok ? (pendingAfter.result as Array<{ id: string }>) : []
    ).some((row) => row.id === E2E_APPROVAL_ID);
    expect(stillPending).toBe(false);

    // Second decision must fail safely (already consumed / not pending).
    const secondDecision = await invokeFromCurrentWindow(
      "kernel_decide_approval",
      { approvalId: E2E_APPROVAL_ID, approve: true },
    );
    expect(secondDecision.ok).toBe(false);
    expect(secondDecision.ok === false && secondDecision.error).toBeTruthy();

    const auditOutcome = await invokeFromCurrentWindow(
      "kernel_list_audit_events",
      { applicationId: null, limit: 50 },
    );
    expect(auditOutcome.ok).toBe(true);
    const audit = auditOutcome.ok
      ? (auditOutcome.result as Array<{ approvalId?: string | null; outcome?: string }>)
      : [];
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
