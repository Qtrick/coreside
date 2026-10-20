/**
 * Unexpected-crash Phase A — start progressive generation, wait for checkpoint
 * paint, write state, then sleep. The supervisor SIGKILLs the tracked E2E app
 * while the turn is still in flight (not a user Cancel).
 */
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

const statePath =
  process.env.CORESIDE_UNEXPECTED_CRASH_STATE?.trim() ||
  path.resolve(E2E_REPO_ROOT, "reports/unexpected-crash-state.json");

describe("Unexpected crash Phase A — leave generation in flight", () => {
  it("stages a progressive checkpoint then waits for supervisor kill", async () => {
    requireExistingSeed("unexpected-crash Phase A");
    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

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

    const badge = await $('[data-testid="tool-preview-badge"]');
    await badge.waitForExist({
      timeout: 8_000,
      timeoutMsg: "Preview badge never appeared before crash window",
    });
    const note = await $(`#${E2E_NOTE_INPUT_ID}`);
    if (await note.isExisting()) {
      await browser.waitUntil(
        async () => {
          const value = await note.getValue();
          return value.toLowerCase().includes("progressive preview note");
        },
        {
          timeout: 4_000,
          timeoutMsg: "checkpoint paint never landed before crash window",
        },
      );
    }

    // Do NOT cancel — generation remains in flight for SIGKILL.
    const cancel = await $('[aria-label="Cancel request"]');
    expect(await cancel.isExisting()).toBe(true);

    const dbPath = process.env.CORESIDE_DB_PATH || "";
    expect(dbPath.length).toBeGreaterThan(0);

    fs.mkdirSync(path.dirname(statePath), { recursive: true });
    fs.writeFileSync(
      statePath,
      JSON.stringify(
        {
          phase: "A",
          mode: "in_flight_awaiting_sigkill",
          writtenAt: new Date().toISOString(),
          dbPath,
          cancelledByUser: false,
        },
        null,
        2,
      ),
    );

    // Supervisor kills the WebDriver holder; this pause is cut short by SIGKILL.
    await browser.pause(180_000);
    throw new Error("Supervisor never SIGKILLed the in-flight E2E process");
  });
});
