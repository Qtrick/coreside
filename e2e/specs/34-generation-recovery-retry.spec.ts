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

async function cancelProgressiveHold() {
  const badge = await $('[data-testid="tool-preview-badge"]');
  await badge.waitForExist({
    timeout: 8_000,
    timeoutMsg: "Preview badge never appeared (mock progressive surface?)",
  });
  // Wait for paint (and therefore checkpoint staging) before cancel — badge alone
  // can race ahead of turn_journal.operations_json writes.
  const note = await $(`#${E2E_NOTE_INPUT_ID}`);
  if (await note.isExisting()) {
    await browser.waitUntil(
      async () => {
        const value = await note.getValue();
        return value.toLowerCase().includes("progressive preview note");
      },
      {
        timeout: 4_000,
        timeoutMsg: "preview state never painted before cancel",
      },
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
      // Banner refresh is event-driven; nudge focus so a missed Channel event
      // still reloads list_recoverable_generations.
      await browser.execute(() => window.dispatchEvent(new Event("focus")));
      return recovery.isExisting();
    },
    {
      timeout,
      interval: 500,
      timeoutMsg: "Recovery banner never appeared after cancel",
    },
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

describe("Journey 34 — generation recovery retry", () => {
  it("reviews recovered preview then retries without losing staging on cancel", async () => {
    requireExistingSeed("Journey 34");
    const startedAt = new Date().toISOString();
    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);
    // Prior suite runs leave recovery cards in the seeded Biology chat.
    await discardAllRecoveries();

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

    await cancelProgressiveHold();
    const recovery = await waitForRecoveryBanner();

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
    await close.waitForExist({ timeout: 5_000 });
    await close.scrollIntoView();
    try {
      await close.waitForClickable({ timeout: 5_000 });
      await close.click();
    } catch {
      // Overlay / sticky composer can intercept hit-testing; force the close.
      await browser.execute((el) => (el as HTMLElement).click(), close);
    }
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

  it("busy reply rejects recovery retry without clearing staging", async () => {
    requireExistingSeed("Journey 34 busy");
    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);
    await discardAllRecoveries();

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    await composer.setValue("Please run progressive surface preview now");
    const send = await $('[aria-label="Send message"]');
    await send.waitForClickable({ timeout: 5_000 });
    await send.click();

    await cancelProgressiveHold();
    const recovery = await waitForRecoveryBanner();

    // Start a second progressive hold so the chat is busy, then Retry.
    await composer.click();
    await composer.setValue("Please run progressive surface preview again");
    await send.waitForClickable({ timeout: 5_000 });
    await send.click();
    const badge = await $('[data-testid="tool-preview-badge"]');
    await badge.waitForExist({ timeout: 8_000 });

    const retry = await $('button=Retry Generation');
    await retry.waitForClickable({ timeout: 5_000 });
    await retry.click();

    await browser.waitUntil(
      async () => {
        const err = await $('[data-testid="generation-recovery"] .generation-recovery-error');
        if (!(await err.isExisting())) return false;
        const text = (await err.getText()).toLowerCase();
        return text.includes("busy") || text.includes("remains available");
      },
      {
        timeout: 15_000,
        timeoutMsg: "Busy retry did not surface a preserved-staging error",
      },
    );
    expect(await recovery.isExisting()).toBe(true);

    // Cancel the in-flight hold so later tests are not blocked.
    const cancelBusy = await $('[aria-label="Cancel request"]');
    if (await cancelBusy.isExisting()) {
      await cancelBusy.click();
    }
  });

  it("retries a progressive hold into a kernel proposal", async () => {
    requireExistingSeed("Journey 34 proposal");
    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);
    await discardAllRecoveries();

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    // Progressive paint + task tracker → ApplicationPlan after the hold.
    // Retry reuses this content, so cancel then Retry must stage a proposal.
    await composer.setValue(
      "Please run progressive surface preview and build a simple task tracker",
    );
    const send = await $('[aria-label="Send message"]');
    await send.waitForClickable({ timeout: 5_000 });
    await send.click();

    await cancelProgressiveHold();
    await waitForRecoveryBanner();
    const retry = await $('button=Retry Generation');
    await retry.waitForClickable({ timeout: 5_000 });
    await retry.click();

    await browser.waitUntil(
      async () => {
        const apply = await $('button=Apply');
        if (await apply.isExisting()) return true;
        const body = await $("body");
        const text = (await body.getText()).toLowerCase();
        return text.includes("task tracker") && text.includes("apply");
      },
      {
        timeout: 60_000,
        timeoutMsg: "Recovery retry did not surface a kernel proposal Apply",
      },
    );
  });

  it("discards recoverable staging explicitly", async () => {
    requireExistingSeed("Journey 34 discard");
    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);
    await discardAllRecoveries();

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    await composer.setValue("Please run progressive surface preview now");
    const send = await $('[aria-label="Send message"]');
    await send.waitForClickable({ timeout: 5_000 });
    await send.click();

    await cancelProgressiveHold();
    await waitForRecoveryBanner();
    await discardAllRecoveries();

    await browser.waitUntil(
      async () => {
        const recovery = await $('[data-testid="generation-recovery"]');
        return !(await recovery.isExisting());
      },
      {
        timeout: 10_000,
        timeoutMsg: "Recovery banner remained after Discard",
      },
    );
  });
});
