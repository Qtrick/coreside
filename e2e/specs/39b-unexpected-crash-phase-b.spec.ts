/**
 * Unexpected-crash Phase B — fresh process on the same DB after SIGKILL.
 * Startup mark_interrupted_in_flight must surface recoverable generation.
 */
import fs from "node:fs";
import path from "node:path";
import {
  E2E_REPO_ROOT,
  denyPendingApprovalIfPresent,
  evidenceBinaryHash,
  evidenceIdentity,
  openPersonalTool,
  requireExistingSeed,
  waitForAppReady,
  waitForToolCanvas,
  E2E_TOOL_ID,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/unexpected-crash-recovery-results.json",
);
const statePath =
  process.env.CORESIDE_UNEXPECTED_CRASH_STATE?.trim() ||
  path.resolve(E2E_REPO_ROOT, "reports/unexpected-crash-state.json");

describe("Unexpected crash Phase B — recover after SIGKILL", () => {
  it("shows recoverable generation after hard process kill", async () => {
    requireExistingSeed("unexpected-crash Phase B");
    const startedAt = new Date().toISOString();
    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

    const prior = JSON.parse(fs.readFileSync(statePath, "utf8"));
    expect(prior.cancelledByUser).toBe(false);
    expect(prior.mode).toBe("in_flight_awaiting_sigkill");

    const recovery = await $('[data-testid="generation-recovery"]');
    await browser.waitUntil(
      async () => {
        await browser.execute(() => window.dispatchEvent(new Event("focus")));
        return recovery.isExisting();
      },
      {
        timeout: 25_000,
        interval: 500,
        timeoutMsg:
          "Recovery banner missing after unexpected crash — mark_interrupted_in_flight?",
      },
    );

    const review = await $('button=Review Preview');
    await review.waitForClickable({ timeout: 5_000 });
    await review.click();

    const host = await $('[data-testid="recovered-preview-host"]');
    await host.waitForExist({
      timeout: 10_000,
      timeoutMsg: "Recovered preview host never mounted after crash",
    });
    // Checkpoint must never authorize Apply.
    const applyInHost = await host.$("button=Apply");
    expect(await applyInHost.isExisting()).toBe(false);

    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          status: "passed",
          harness: "unexpected-crash-recovery",
          mode: "sigkill_mid_generation",
          evidenceLevel: "Full OS Process Crash Relaunch Verified",
          cancelledByUser: false,
          recoveryVisible: true,
          reviewPreviewReadOnly: true,
          startedAt,
          finishedAt: new Date().toISOString(),
          identity: evidenceIdentity(),
          binaryHash: evidenceBinaryHash(),
          prior,
          e2eSpec: "e2e/specs/39b-unexpected-crash-phase-b.spec.ts",
        },
        null,
        2,
      ),
    );
  });
});
