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
 * Journey 19 — progressive generated-surface preview (Desktop Verified path).
 *
 * Requires AI_PROVIDER=mock and CORESIDE_E2E_SEED=existing.
 * Mock keyword: "progressive surface preview" streams a paint-capable state.set
 * for surf-tool-e2e-notes before ResponseCompleted.
 *
 * Intentionally does NOT use Partial Update HTML/JS execution.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock CORESIDE_E2E_SEED=existing \
 *     npx wdio run e2e/wdio.conf.ts --suite progressive-surface-preview
 */
const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/progressive-preview-results.json",
);

describe("Journey 19 — progressive surface preview", () => {
  it("shows a speculative Preview badge before the turn commits", async () => {
    requireExistingSeed("Journey 19");
    const startedAt = new Date().toISOString();
    const t0 = Date.now();
    let firstBadgeMs: number | null = null;
    let firstPaintMs: number | null = null;
    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    await composer.setValue("Please run progressive surface preview now");

    const send = await $('[aria-label="Send message"]');
    await send.waitForExist({ timeout: 10_000 });
    await send.waitForClickable({ timeout: 5_000 });
    await send.click();
    const turnStartedMs = Date.now();

    const badge = await $('[data-testid="tool-preview-badge"]');
    await badge.waitForExist({
      timeout: 8_000,
      timeoutMsg:
        "Preview badge never appeared before completion (is AI_PROVIDER=mock + progressive surface fixture live?)",
    });
    firstBadgeMs = Date.now() - turnStartedMs;

    const note = await $(`#${E2E_NOTE_INPUT_ID}`);
    await note.waitForExist({
      timeout: 5_000,
      timeoutMsg: "E2E Notes input missing after opening seeded tool",
    });
    await browser.waitUntil(
      async () => {
        const value = await note.getValue();
        return value.toLowerCase().includes("progressive preview note");
      },
      {
        timeout: 4_000,
        timeoutMsg: "preview state never painted note value",
      },
    );
    firstPaintMs = Date.now() - turnStartedMs;
    // Capture painted value before completion — must not wait for final message first.
    const paintedBeforeCompletion = await note.getValue();
    expect(paintedBeforeCompletion.toLowerCase()).toContain(
      "progressive preview note",
    );
    expect(await badge.isExisting()).toBe(true);

    await browser.waitUntil(
      async () => {
        const body = await $("body");
        const text = (await body.getText()).toLowerCase();
        return text.includes("progressive surface preview complete");
      },
      {
        timeout: 20_000,
        timeoutMsg: "turn never completed with progressive surface preview message",
      },
    );
    const completionMs = Date.now() - turnStartedMs;

    // After commit, speculative badge should clear.
    await browser.waitUntil(
      async () => !(await badge.isExisting()),
      {
        timeout: 10_000,
        timeoutMsg: "Preview badge remained after durable commit",
      },
    );

    const finalNote = await note.getValue();
    expect(finalNote.toLowerCase()).toContain("progressive preview note");

    const identity = evidenceIdentity();
    const evidence = {
      schemaVersion: 1,
      journey: 19,
      name: "progressive-surface-preview",
      evidenceLevel: "Desktop Verified",
      startedAt,
      endedAt: new Date().toISOString(),
      generatedAt: new Date().toISOString(),
      commit: identity.commit,
      dirty: identity.dirty,
      sourceFingerprint: identity.sourceFingerprint,
      binaryHash: evidenceBinaryHash(),
      e2eSpec: "e2e/specs/19-progressive-surface-preview.spec.ts",
      providerFixture: "mock progressive surface preview",
      assertions: [
        "preview badge visible before completion",
        "note input painted progressive preview value before completion",
        "completion message received",
        "preview badge cleared after commit",
        "final note value remains progressive preview note",
      ],
      metrics: {
        turnStartToFirstBadgeMs: firstBadgeMs,
        turnStartToFirstPaintMs: firstPaintMs,
        turnStartToCompletionMs: completionMs,
        totalWallMs: Date.now() - t0,
        note:
          "Timing is observational (not a CI gate). SQLite write absence during speculation is Unit Verified separately — do not elevate that claim from this Desktop run.",
      },
      intentionallyRejected: [
        "PU-HTML-RESP",
        "PU-JS",
        "PU-CDN",
        "PU-FORM-ROUTE",
        "PU-SCOPED-CSS",
      ],
    };
    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
  });
});
