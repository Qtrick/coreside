/**
 * Journey 32 — Progressive repair recovery.
 *
 * Mock path: first "progressive repair recovery" emits an ApplicationPlan with
 * empty intents (parse/validate rejects). Retry with "after repair" emits a
 * valid create plan → Apply → Repair Probe lands durably.
 *
 * Interactive.ai_turn illegal→repair success is covered by mock Rust unit tests
 * (interactive_ai_turn_illegal_then_repair_proposes_action_id).
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock CORESIDE_E2E_SEED=empty \
 *     npx wdio run e2e/wdio.conf.ts --suite progressive-repair-recovery
 */
import fs from "node:fs";
import path from "node:path";
import {
  E2E_REPO_ROOT,
  denyPendingApprovalIfPresent,
  evidenceBinaryHash,
  evidenceIdentity,
  latestVisibleKernelApplyButton,
  waitForAppReady,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/progressive-repair-recovery-results.json",
);

describe("Journey 32 — progressive repair recovery", () => {
  it("fails validation then recovers with a durable Apply", async () => {
    const startedAt = new Date().toISOString();
    const t0 = Date.now();
    await waitForAppReady();
    await denyPendingApprovalIfPresent();

    const newChat = await $('[aria-label="New chat"]');
    await newChat.waitForClickable({ timeout: 15_000 });
    await newChat.click();

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    await composer.setValue("Please run progressive repair recovery now");

    const send = await $('[aria-label="Send message"]');
    await send.waitForClickable({ timeout: 10_000 });
    await send.click();

    // Validate/compile failure: no Apply, and a parse/validation alert (not the user prompt).
    // Do not match bare "repair" — the typed prompt itself contains that word.
    await browser.waitUntil(
      async () => {
        if ((await latestVisibleKernelApplyButton()) !== null) {
          return false;
        }
        const alerts = await $$('[role="alert"]');
        for (const alert of alerts) {
          if (!(await alert.isExisting()) || !(await alert.isDisplayed())) {
            continue;
          }
          const text = (await alert.getText()).toLowerCase();
          if (
            text.includes("intent") ||
            text.includes("empty") ||
            text.includes("rejected") ||
            text.includes("applicationplan")
          ) {
            return true;
          }
        }
        return false;
      },
      {
        timeout: 45_000,
        timeoutMsg:
          "Expected validation failure alert after progressive repair recovery (no Apply)",
      },
    );
    const applyAfterFail = await latestVisibleKernelApplyButton();
    if (applyAfterFail) {
      throw new Error(
        "Kernel Apply appeared after intentional validation failure",
      );
    }

    await composer.click();
    await composer.setValue(
      "progressive repair recovery after repair — ready after repair",
    );
    await send.waitForClickable({ timeout: 10_000 });
    await send.click();

    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) !== null,
      {
        timeout: 45_000,
        timeoutMsg:
          "Repair success Apply never appeared (progressive_repair_recovery_success)",
      },
    );
    const applyBtn = await latestVisibleKernelApplyButton();
    if (!applyBtn) {
      throw new Error("Repair success Apply missing after wait");
    }
    await applyBtn.waitForClickable({ timeout: 10_000 });
    await applyBtn.click();

    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) === null,
      {
        timeout: 25_000,
        timeoutMsg: "Repair Apply still visible — kernel decide likely failed",
      },
    );

    await browser.waitUntil(
      async () => {
        const apps = await $('section[aria-label="Personal apps"]');
        if (await apps.isExisting()) {
          const btn = await apps.$('button[aria-label="Repair Probe"]');
          if (await btn.isExisting()) return true;
        }
        const body = (await $("body").getText()).toLowerCase();
        return (
          body.includes("repair probe") ||
          body.includes("repair recovery complete")
        );
      },
      {
        timeout: 25_000,
        timeoutMsg: "Repair Probe never landed after successful repair Apply",
      },
    );

    const finishedAt = new Date().toISOString();
    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          status: "passed",
          journey: 32,
          name: "progressive-repair-recovery",
          protocol: "applicationPlan",
          path: "validate_fail_then_repair_success",
          firstTurnApplyAbsent: true,
          repairApplySucceeded: true,
          startedAt,
          finishedAt,
          durationMs: Date.now() - t0,
          identity: evidenceIdentity(),
          binaryHash: evidenceBinaryHash(),
          e2eSpec: "e2e/specs/32-progressive-repair-recovery.spec.ts",
          providerFixture: "mock progressive repair recovery",
        },
        null,
        2,
      ),
    );
  });
});
