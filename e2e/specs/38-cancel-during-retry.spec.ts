/**
 * Journey 38 — Cancel during recovery Retry.
 *
 * Create recoverable generation → Retry → cancel the replacement stream →
 * verify original prompt remains, recovery reconciles, later retry still works.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock CORESIDE_E2E_SEED=existing \
 *     npx wdio run e2e/wdio.conf.ts --suite cancel-during-retry
 */
import fs from "node:fs";
import path from "node:path";
import {
  E2E_NOTE_INPUT_ID,
  E2E_REPO_ROOT,
  E2E_TOOL_ID,
  denyPendingApprovalIfPresent,
  evidenceBinaryHash,
  evidenceIdentity,
  openPersonalTool,
  requireExistingSeed,
  waitForAppReady,
  waitForToolCanvas,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/cancel-during-retry-results.json",
);

const PROMPT = "Please run progressive surface preview now";

async function cancelProgressiveHold() {
  const badge = await $('[data-testid="tool-preview-badge"]');
  await badge.waitForExist({
    timeout: 8_000,
    timeoutMsg: "Preview badge never appeared",
  });
  const note = await $(`#${E2E_NOTE_INPUT_ID}`);
  if (await note.isExisting()) {
    await browser.waitUntil(
      async () => {
        const value = await note.getValue();
        return value.toLowerCase().includes("progressive preview note");
      },
      { timeout: 4_000, timeoutMsg: "preview state never painted before cancel" },
    );
  }
  const cancel = await $('[aria-label="Cancel request"]');
  await cancel.waitForClickable({ timeout: 3_000 });
  await cancel.click();
}

async function waitForRecoveryBanner(timeout = 20_000) {
  const recovery = await $('[data-testid="generation-recovery"]');
  await browser.waitUntil(
    async () => {
      await browser.execute(() => window.dispatchEvent(new Event("focus")));
      return recovery.isExisting();
    },
    { timeout, interval: 500, timeoutMsg: "Recovery banner never appeared" },
  );
  return recovery;
}

async function discardAllRecoveries() {
  for (let i = 0; i < 8; i += 1) {
    const recovery = await $('[data-testid="generation-recovery"]');
    if (!(await recovery.isExisting())) return;
    const discard = await $('button=Discard Preview');
    if (!(await discard.isExisting())) return;
    await discard.waitForClickable({ timeout: 5_000 });
    await discard.click();
    await browser.pause(300);
  }
}

describe("Journey 38 — cancel during recovery retry", () => {
  it("cancels a replacement retry stream without losing the original message", async () => {
    requireExistingSeed("Journey 38");
    const startedAt = new Date().toISOString();
    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);
    await discardAllRecoveries();

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    await composer.setValue(PROMPT);

    const send = await $('[aria-label="Send message"]');
    await send.waitForClickable({ timeout: 5_000 });
    await send.click();

    await cancelProgressiveHold();
    await waitForRecoveryBanner();

    const retry = await $('button=Retry Generation');
    await retry.waitForClickable({ timeout: 5_000 });
    await retry.click();

    // Replacement attempt should start streaming (preview badge again).
    const badge = await $('[data-testid="tool-preview-badge"]');
    await badge.waitForExist({
      timeout: 30_000,
      timeoutMsg: "Retry never started a replacement progressive preview",
    });

    const cancel = await $('[aria-label="Cancel request"]');
    await cancel.waitForClickable({ timeout: 5_000 });
    await cancel.click();

    const recoveryAgain = await waitForRecoveryBanner(25_000);
    expect(await recoveryAgain.isExisting()).toBe(true);

    const body = await $("body");
    const transcript = await body.getText();
    const hits = transcript.split(PROMPT).length - 1;
    expect(hits).toBeGreaterThanOrEqual(1);
    expect(hits).toBeLessThanOrEqual(2);

    // Later retry still works.
    const retry2 = await $('button=Retry Generation');
    await retry2.waitForClickable({ timeout: 5_000 });
    await retry2.click();

    await browser.waitUntil(
      async () => {
        const still = await $('[data-testid="generation-recovery"]');
        if (await still.isExisting()) {
          const text = (await still.getText()).toLowerCase();
          if (text.includes("remains available")) return false;
        }
        return (await $('[aria-label="Send message"]')).isExisting();
      },
      {
        timeout: 45_000,
        timeoutMsg: "Second retry after cancel-during-retry never admitted",
      },
    );

    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          status: "passed",
          journey: 38,
          name: "cancel-during-retry",
          originalPromptPreserved: true,
          recoveryAfterCancel: true,
          secondRetryAdmitted: true,
          startedAt,
          finishedAt: new Date().toISOString(),
          identity: evidenceIdentity(),
          binaryHash: evidenceBinaryHash(),
          e2eSpec: "e2e/specs/38-cancel-during-retry.spec.ts",
        },
        null,
        2,
      ),
    );
  });
});
