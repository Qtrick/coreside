/**
 * Journey 37 — Cross-surface reactive data sync (canvas ↔ inline summary).
 *
 * After Study Planner Apply, mutations on the tool canvas must refresh the
 * conversation-bound inline summary without requiring another model request.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock CORESIDE_E2E_SEED=empty \
 *     npx wdio run e2e/wdio.conf.ts --suite cross-surface-reactive-sync
 */
import fs from "node:fs";
import path from "node:path";
import {
  E2E_REPO_ROOT,
  approveOnceIfPresent,
  denyPendingApprovalIfPresent,
  evidenceBinaryHash,
  evidenceIdentity,
  clickKernelApplyButton,
  waitForAppReady,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/cross-surface-reactive-sync-results.json",
);

const TASK_A = "Cross-surface sync task A";
const TASK_B = "Cross-surface sync task B";

describe("Journey 37 — cross-surface reactive data sync", () => {
  it("keeps Study Planner canvas and inline summary consistent", async () => {
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
    await composer.setValue("Build me a study planner");

    const send = await $('[aria-label="Send message"]');
    await send.waitForClickable({ timeout: 10_000 });
    await send.click();
    await clickKernelApplyButton();

    const inlineSummary = await $(
      '.inline-surface[data-surface-id="s-study-planner-summary"]',
    );
    await inlineSummary.waitForExist({ timeout: 25_000 });

    const appsSection = await $('section[aria-label="Personal apps"]');
    await appsSection.waitForExist({ timeout: 20_000 });
    const toolBtn = await appsSection.$('button[aria-label="Study Planner"]');
    const canvasSel = '.tool-canvas-body[data-tool-id="tool-study-planner"]';
    if (!(await $(canvasSel).isExisting())) {
      await toolBtn.waitForClickable({ timeout: 10_000 });
      await toolBtn.click();
    }
    const canvas = await $(canvasSel);
    await canvas.waitForExist({ timeout: 15_000 });

    async function addTask(title: string) {
      const titleEl = await canvas.$("#sp-new-title");
      await titleEl.click();
      await titleEl.setValue(title);
      const addBtn = await canvas.$('button[data-component-id="sp-add-btn"]');
      await addBtn.waitForClickable({ timeout: 10_000 });
      await addBtn.click();
      await approveOnceIfPresent();
      await browser.waitUntil(
        async () => (await canvas.getText()).includes(title),
        { timeout: 20_000, timeoutMsg: `Task ${title} missing from canvas` },
      );
    }

    await addTask(TASK_A);
    await browser.waitUntil(
      async () => {
        const t = (await inlineSummary.getText()).toLowerCase();
        return t.includes("1 study task") || t.includes("1 remaining");
      },
      {
        timeout: 15_000,
        timeoutMsg: "Inline summary did not refresh after first create",
      },
    );

    await addTask(TASK_B);
    await browser.waitUntil(
      async () => {
        const t = (await inlineSummary.getText()).toLowerCase();
        return t.includes("2 study task") || t.includes("2 remaining");
      },
      {
        timeout: 15_000,
        timeoutMsg: "Inline summary did not refresh after second create",
      },
    );

    const row = await canvas.$(`td*=${TASK_A}`);
    await row.click();
    const doneBtn = await canvas.$('button[data-component-id="sp-done-btn"]');
    await doneBtn.click();
    await approveOnceIfPresent();

    await browser.waitUntil(
      async () => {
        const t = (await inlineSummary.getText()).toLowerCase();
        return t.includes("1 completed") && t.includes("1 remaining");
      },
      {
        timeout: 15_000,
        timeoutMsg: "Inline summary did not refresh after completion",
      },
    );

    // Navigate away (close canvas) and return — both hydrate from SQLite.
    const close = await $(
      '[aria-label="Close tool canvas"], [aria-label="Back to chat"]',
    );
    if (await close.isExisting()) {
      await close.click();
    }
    await browser.waitUntil(
      async () => !(await $(canvasSel).isExisting()),
      { timeout: 10_000, timeoutMsg: "Canvas did not close" },
    );

    await browser.waitUntil(
      async () => {
        const t = (await inlineSummary.getText()).toLowerCase();
        return t.includes("1 completed") && t.includes("1 remaining");
      },
      {
        timeout: 15_000,
        timeoutMsg: "Inline summary lost counts after canvas close",
      },
    );

    await toolBtn.waitForClickable({ timeout: 10_000 });
    await toolBtn.click();
    const canvasAgain = await $(canvasSel);
    await canvasAgain.waitForExist({ timeout: 15_000 });
    await browser.waitUntil(
      async () => {
        const text = await canvasAgain.getText();
        return text.includes(TASK_A) && text.includes(TASK_B);
      },
      {
        timeout: 15_000,
        timeoutMsg: "Canvas missing tasks after reopen",
      },
    );

    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          status: "passed",
          journey: 37,
          name: "cross-surface-reactive-sync",
          createSynced: true,
          completeSynced: true,
          remountHydrated: true,
          startedAt,
          finishedAt: new Date().toISOString(),
          durationMs: Date.now() - t0,
          identity: evidenceIdentity(),
          binaryHash: evidenceBinaryHash(),
          e2eSpec: "e2e/specs/37-cross-surface-reactive-sync.spec.ts",
        },
        null,
        2,
      ),
    );
  });
});
