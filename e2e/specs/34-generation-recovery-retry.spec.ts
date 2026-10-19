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

/**
 * Journey 34 — generation recovery review / retry / discard.
 *
 * Requires AI_PROVIDER=mock and CORESIDE_E2E_SEED=existing.
 * Uses the progressive surface preview mock hold, cancels mid-flight, then
 * exercises Review Preview → Close → Retry Generation on the recovery banner.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock CORESIDE_E2E_SEED=existing \
 *     npx wdio run e2e/wdio.conf.ts --suite generation-recovery-retry
 */
const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/generation-recovery-retry-results.json",
);

const ORIGINAL_NOTE = "original e2e note before recovery";

describe("Journey 34 — generation recovery retry", () => {
  it("reviews recovered preview then retries without losing staging on cancel", async () => {
    requireExistingSeed("Journey 34");
    const startedAt = new Date().toISOString();
    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

    const note = await $(`#${E2E_NOTE_INPUT_ID}`);
    await note.waitForExist({ timeout: 5_000 });
    await note.setValue(ORIGINAL_NOTE);

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    await composer.setValue("Please run progressive surface preview now");

    const send = await $('[aria-label="Send message"]');
    await send.waitForClickable({ timeout: 5_000 });
    await send.click();

    const badge = await $('[data-testid="tool-preview-badge"]');
    await badge.waitForExist({
      timeout: 8_000,
      timeoutMsg: "Preview badge never appeared (mock progressive surface?)",
    });

    const cancel = await $('[aria-label="Cancel request"]');
    await cancel.waitForClickable({ timeout: 3_000 });
    await cancel.click();

    const recovery = await $('[data-testid="generation-recovery"]');
    await recovery.waitForExist({
      timeout: 15_000,
      timeoutMsg: "Recovery banner never appeared after cancel",
    });

    const review = await $('button=Review Preview');
    await review.waitForClickable({ timeout: 5_000 });
    await review.click();

    const host = await $('[data-testid="recovered-preview-host"]');
    await host.waitForExist({
      timeout: 10_000,
      timeoutMsg: "Recovered preview host never mounted",
    });
    expect(await host.getAttribute("data-fidelity")).toBeDefined();

    const close = await $('[aria-label="Close recovered preview"]');
    await close.waitForClickable({ timeout: 5_000 });
    await close.click();
    await browser.waitUntil(async () => !(await host.isExisting()), {
      timeout: 5_000,
      timeoutMsg: "Recovered preview host remained after Close",
    });

    // Staging must still be listed after closing review (not discarded).
    await recovery.waitForExist({ timeout: 5_000 });

    const retry = await $('button=Retry Generation');
    await retry.waitForClickable({ timeout: 5_000 });
    await retry.click();

    // After admitted retry, recovery for that turn should clear; send returns.
    await browser.waitUntil(
      async () => {
        const still = await $('[data-testid="generation-recovery"]');
        if (await still.isExisting()) {
          const text = (await still.getText()).toLowerCase();
          if (text.includes("remains available")) return false;
        }
        const sendAgain = await $('[aria-label="Send message"]');
        return sendAgain.isExisting();
      },
      {
        timeout: 45_000,
        timeoutMsg: "Retry did not admit a replacement generation",
      },
    );

    const evidence = {
      journey: 34,
      name: "generation-recovery-retry",
      startedAt,
      finishedAt: new Date().toISOString(),
      identity: evidenceIdentity(),
      binaryHash: evidenceBinaryHash(),
      status: "passed",
      checks: {
        recoveryBanner: true,
        reviewHostMounted: true,
        closeKeepsStaging: true,
        retryAdmitted: true,
      },
    };
    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));
  });

  it("discards recoverable staging explicitly", async () => {
    requireExistingSeed("Journey 34 discard");
    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    await composer.setValue("Please run progressive surface preview now");
    const send = await $('[aria-label="Send message"]');
    await send.waitForClickable({ timeout: 5_000 });
    await send.click();

    const badge = await $('[data-testid="tool-preview-badge"]');
    await badge.waitForExist({ timeout: 8_000 });
    const cancel = await $('[aria-label="Cancel request"]');
    await cancel.waitForClickable({ timeout: 3_000 });
    await cancel.click();

    const recovery = await $('[data-testid="generation-recovery"]');
    await recovery.waitForExist({ timeout: 15_000 });
    const discard = await $('button=Discard Preview');
    await discard.waitForClickable({ timeout: 5_000 });
    await discard.click();

    await browser.waitUntil(async () => !(await recovery.isExisting()), {
      timeout: 10_000,
      timeoutMsg: "Recovery banner remained after Discard",
    });
  });
});
