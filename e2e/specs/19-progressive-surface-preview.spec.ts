import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
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

    const badge = await $('[data-testid="tool-preview-badge"]');
    await badge.waitForExist({
      timeout: 8_000,
      timeoutMsg:
        "Preview badge never appeared before completion (is AI_PROVIDER=mock + progressive surface fixture live?)",
    });

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

    // After commit, speculative badge should clear.
    await browser.waitUntil(
      async () => !(await badge.isExisting()),
      {
        timeout: 10_000,
        timeoutMsg: "Preview badge remained after durable commit",
      },
    );

    const identity = evidenceIdentity();
    const fingerprint = createHash("sha256")
      .update(fs.readFileSync(path.join(E2E_REPO_ROOT, "package.json")))
      .update(fs.readFileSync(path.join(E2E_REPO_ROOT, "src-tauri/Cargo.lock")))
      .digest("hex");
    const evidence = {
      schemaVersion: 1,
      journey: 19,
      name: "progressive-surface-preview",
      evidenceLevel: "Desktop Verified",
      generatedAt: new Date().toISOString(),
      commit: identity.commit,
      dirty: identity.dirty,
      sourceFingerprint: fingerprint,
      binaryHash: evidenceBinaryHash(),
      e2eSpec: "e2e/specs/19-progressive-surface-preview.spec.ts",
      assertions: [
        "preview badge visible before completion",
        "completion message received",
        "preview badge cleared after commit",
      ],
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
