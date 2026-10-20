import fs from "node:fs";
import path from "node:path";
import {
  E2E_NOTE_INPUT_ID,
  E2E_REPO_ROOT,
  E2E_TOOL_ID,
  denyPendingApprovalIfPresent,
  openPersonalTool,
  requireExistingSeed,
  waitForAppReady,
  waitForToolCanvas,
} from "../helpers.js";

/**
 * Process-relaunch Phase A — create interrupted recoverable staging, then exit.
 * The supervisor (process-relaunch-recovery.mjs) starts Phase B in a NEW process
 * with the same CORESIDE_DB_PATH.
 */
const statePath =
  process.env.CORESIDE_PROCESS_RELAUNCH_STATE?.trim() ||
  path.resolve(E2E_REPO_ROOT, "reports/process-relaunch-state.json");

async function cancelProgressiveHold() {
  const badge = await $('[data-testid="tool-preview-badge"]');
  await badge.waitForExist({
    timeout: 8_000,
    timeoutMsg: "Preview badge never appeared",
  });
  const cancel = await $('[aria-label="Cancel request"]');
  await cancel.waitForClickable({ timeout: 3_000 });
  await cancel.click();
}

describe("Process relaunch Phase A — stage recovery then exit", () => {
  it("interrupts progressive generation and persists recoverable staging", async () => {
    requireExistingSeed("process-relaunch Phase A");
    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

    // Clear leftover recovery cards from the seeded Biology chat.
    for (let i = 0; i < 8; i += 1) {
      const recovery = await $('[data-testid="generation-recovery"]');
      if (!(await recovery.isExisting())) break;
      const discard = await $('button=Discard Preview');
      if (!(await discard.isExisting())) break;
      await discard.click();
      await browser.pause(300);
    }

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    await composer.setValue("Please run progressive surface preview now");
    const send = await $('[aria-label="Send message"]');
    await send.waitForClickable({ timeout: 5_000 });
    await send.click();

    await cancelProgressiveHold();

    const recovery = await $('[data-testid="generation-recovery"]');
    await browser.waitUntil(
      async () => {
        await browser.execute(() => window.dispatchEvent(new Event("focus")));
        return recovery.isExisting();
      },
      {
        timeout: 20_000,
        interval: 500,
        timeoutMsg: "Phase A: recovery banner never appeared",
      },
    );

    const dbPath = process.env.CORESIDE_DB_PATH || "";
    expect(dbPath.length).toBeGreaterThan(0);
    expect(fs.existsSync(dbPath)).toBe(true);

    const state = {
      phase: "A",
      writtenAt: new Date().toISOString(),
      dbPath,
      recoveryVisible: true,
      noteInputId: E2E_NOTE_INPUT_ID,
    };
    fs.mkdirSync(path.dirname(statePath), { recursive: true });
    fs.writeFileSync(statePath, JSON.stringify(state, null, 2));
  });
});
