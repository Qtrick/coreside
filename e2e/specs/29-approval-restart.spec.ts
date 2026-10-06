/**
 * Journey 29 — Pending approval survives durable restart paths.
 *
 * Desktop: seeded pending approval must remain listed by the kernel after a
 * settings navigation remount (durable SQLite authority). Approve once consumes
 * it; a second decide fails.
 *
 * Authority-layer process restart (close + reopen SQLite file, exact call_hash,
 * single-use consume, tampered-input rejection) is covered by
 * `pending_approval_survives_database_reopen_with_exact_call_hash` in
 * `approvals.rs`. Full OS process relaunch inside one WDIO embedded-WebDriver
 * session still kills the driver port; that path remains an environmental limit.
 *
 * Run alone:
 *   CORESIDE_E2E=1 CORESIDE_E2E_SEED=existing \
 *     npx wdio run e2e/wdio.conf.ts --suite approval-restart
 */
import fs from "node:fs";
import path from "node:path";
import {
  E2E_APPROVAL_ID,
  E2E_REPO_ROOT,
  approveOnce,
  closeSettings,
  evidenceBinaryHash,
  evidenceIdentity,
  invokeFromCurrentWindow,
  openSettings,
  requireExistingSeed,
  waitForAppReady,
  waitForPendingApproval,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/approval-restart-results.json",
);

async function pendingApprovalListed(): Promise<boolean> {
  const outcome = await invokeFromCurrentWindow("kernel_list_pending_approvals");
  if (!outcome.ok) return false;
  const rows = outcome.result as Array<{ id: string; status: string }>;
  return rows.some((row) => row.id === E2E_APPROVAL_ID && row.status === "pending");
}

describe("Journey 29 — approval survives restart/remount", () => {
  it("keeps pending approval after remount, approves once, rejects second decide", async () => {
    requireExistingSeed("Journey 29");
    const startedAt = new Date().toISOString();
    const t0 = Date.now();

    await waitForAppReady();

    await browser.waitUntil(async () => pendingApprovalListed(), {
      timeout: 20_000,
      timeoutMsg: "Seeded pending approval never listed by kernel",
    });
    await waitForPendingApproval();

    // Remount shell surfaces (settings open/close) without killing the process.
    await openSettings();
    await closeSettings();

    await browser.waitUntil(async () => pendingApprovalListed(), {
      timeout: 15_000,
      timeoutMsg: "Pending approval missing from kernel after settings remount",
    });
    await waitForPendingApproval();

    await approveOnce();

    await browser.waitUntil(
      async () => !(await pendingApprovalListed()),
      {
        timeout: 15_000,
        timeoutMsg: "Approval still pending after Approve once",
      },
    );

    const second = await invokeFromCurrentWindow("kernel_decide_approval", {
      approvalId: E2E_APPROVAL_ID,
      approve: true,
    });
    expect(second.ok).toBe(false);
    expect(String(second.ok ? "" : second.error).toLowerCase()).toMatch(
      /already|not pending|consumed|decided|status|expired|invalid|not found/,
    );

    const identity = evidenceIdentity();
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          journey: 29,
          name: "approval-restart",
          status: "passed",
          startedAt,
          finishedAt: new Date().toISOString(),
          durationMs: Date.now() - t0,
          approvalId: E2E_APPROVAL_ID,
          processRelaunch: false,
          remountPath: "settings-open-close",
          secondDecideRejected: !second.ok,
          ...identity,
          binarySha256: evidenceBinaryHash(),
        },
        null,
        2,
      ),
    );
  });
});
