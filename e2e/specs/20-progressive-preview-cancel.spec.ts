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
 * Journey 20 — progressive preview cancel / rollback (Desktop Verified path).
 *
 * Requires AI_PROVIDER=mock and CORESIDE_E2E_SEED=existing.
 * Mock keyword: "progressive surface preview" paints then holds (~2s) so the
 * real composer Cancel control can interrupt before durable complete.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock CORESIDE_E2E_SEED=existing \
 *     npx wdio run e2e/wdio.conf.ts --suite progressive-preview-cancel
 */
const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/progressive-preview-cancel-results.json",
);

const ORIGINAL_NOTE = "original e2e note before preview";

describe("Journey 20 — progressive preview cancel", () => {
  it("cancels speculative preview and rolls note back without a stuck queue", async () => {
    requireExistingSeed("Journey 20");
    const startedAt = new Date().toISOString();
    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

    const note = await $(`#${E2E_NOTE_INPUT_ID}`);
    await note.waitForExist({
      timeout: 5_000,
      timeoutMsg: "E2E Notes input missing after opening seeded tool",
    });
    await note.setValue(ORIGINAL_NOTE);
    await browser.waitUntil(
      async () => (await note.getValue()) === ORIGINAL_NOTE,
      {
        timeout: 5_000,
        timeoutMsg: "failed to seed original note value before preview",
      },
    );

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    await composer.setValue("Please run progressive surface preview now");

    const send = await $('[aria-label="Send message"]');
    await send.waitForExist({ timeout: 10_000 });
    await send.waitForClickable({ timeout: 5_000 });
    await send.click();

    const badge = await $('[data-testid="tool-preview-badge"]');
    await badge.waitForExist({
      timeout: 8_000,
      timeoutMsg:
        "Preview badge never appeared before cancel (is AI_PROVIDER=mock + progressive surface fixture live?)",
    });

    await browser.waitUntil(
      async () => {
        const value = await note.getValue();
        return value.toLowerCase().includes("progressive preview note");
      },
      {
        timeout: 4_000,
        timeoutMsg: "preview state never painted note value before cancel",
      },
    );

    // Cancel immediately after paint — mock hold is finite; do not insert sleeps.
    const cancel = await $('[aria-label="Cancel request"]');
    await cancel.waitForExist({
      timeout: 3_000,
      timeoutMsg: "Cancel request control never appeared while sending",
    });
    await cancel.waitForClickable({ timeout: 2_000 });
    await cancel.click();

    await browser.waitUntil(
      async () => !(await badge.isExisting()),
      {
        timeout: 10_000,
        timeoutMsg: "Preview badge remained after cancel",
      },
    );

    await browser.waitUntil(
      async () => (await note.getValue()) === ORIGINAL_NOTE,
      {
        timeout: 10_000,
        timeoutMsg: "note did not roll back to original after cancel",
      },
    );

    // Composer must leave sending state (no stuck Cancel / missing Send).
    const sendAgain = await $('[aria-label="Send message"]');
    await sendAgain.waitForExist({
      timeout: 10_000,
      timeoutMsg: "Send message never returned after cancel (stuck sending?)",
    });
    await browser.waitUntil(
      async () => !(await $('[aria-label="Cancel request"]').isExisting()),
      {
        timeout: 5_000,
        timeoutMsg: "Cancel request control remained after cancel",
      },
    );

    // Queue should not retain a stuck running/queued item for this turn.
    const queueStuck = await browser.execute(() => {
      const queue = document.querySelector('[aria-label="Conversation queue"]');
      if (!queue) return false;
      const text = (queue.textContent ?? "").toLowerCase();
      return (
        text.includes("running") ||
        text.includes("queued") ||
        Boolean(
          queue.querySelector(
            '[aria-label^="Cancel queued message:"], [aria-label*="Running"]',
          ),
        )
      );
    });
    expect(queueStuck).toBe(false);

    // Durable complete must not have won the race (assistant completion only).
    const body = await $("body");
    const bodyText = (await body.getText()).toLowerCase();
    expect(bodyText).not.toContain("progressive surface preview complete");

    // Confirm durable note remains the seeded original (not a stale preview value).
    expect(await note.getValue()).toBe(ORIGINAL_NOTE);

    const identity = evidenceIdentity();
    const evidence = {
      schemaVersion: 1,
      journey: 20,
      name: "progressive-preview-cancel",
      evidenceLevel: "Desktop Verified",
      startedAt,
      endedAt: new Date().toISOString(),
      generatedAt: new Date().toISOString(),
      commit: identity.commit,
      dirty: identity.dirty,
      sourceFingerprint: identity.sourceFingerprint,
      binaryHash: evidenceBinaryHash(),
      e2eSpec: "e2e/specs/20-progressive-preview-cancel.spec.ts",
      providerFixture: "mock progressive surface preview (cancel during hold)",
      assertions: [
        "preview badge visible before cancel",
        "preview note painted before cancel",
        "cancel via real Cancel request control",
        "preview badge cleared after cancel",
        "note rolled back to original",
        "composer left sending state",
        "conversation queue not stuck",
        "durable complete message absent",
      ],
      intentionallyRejected: [
        "PU-HTML-RESP",
        "PU-JS",
        "PU-CDN",
        "fake frontend state mutation",
      ],
    };
    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
  });
});
