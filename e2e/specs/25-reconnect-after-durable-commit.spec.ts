/**
 * Journey 25 — Reconnect after durable commit before renderer acknowledgement.
 *
 * Evolve while the Tool Canvas is closed (no live mount / renderer ack), then
 * reopen + reconnect so catch-up rehydrates the durable definition + records.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock \
 *     npx wdio run e2e/wdio.conf.ts --suite reconnect-after-durable-commit
 */
import fs from "node:fs";
import path from "node:path";
import {
  E2E_REPO_ROOT,
  closeToolCanvas,
  denyPendingApprovalIfPresent,
  evidenceBinaryHash,
  evidenceIdentity,
  latestVisibleKernelApplyButton,
  simulateConversationReconnect,
  waitForAppReady,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/reconnect-after-durable-commit-results.json",
);

const TASK_TITLE = "Committed before renderer ack";

describe("Journey 25 — reconnect after durable commit before renderer ack", () => {
  it("rehydrates evolved surface after unmounted durable commit", async () => {
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
    await composer.setValue("Build me a simple task tracker");

    const send = await $('[aria-label="Send message"]');
    await send.waitForClickable({ timeout: 10_000 });
    await send.click();

    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) !== null,
      { timeout: 45_000, timeoutMsg: "Create Apply never appeared" },
    );
    const createApply = await latestVisibleKernelApplyButton();
    if (!createApply) throw new Error("Create Apply missing");
    await createApply.waitForClickable({ timeout: 10_000 });
    await createApply.click();

    const appsSection = await $('section[aria-label="Personal apps"]');
    await appsSection.waitForExist({ timeout: 20_000 });
    await browser.waitUntil(
      async () =>
        (await appsSection.$('button[aria-label="Task Tracker"]')).isExisting(),
      { timeout: 20_000, timeoutMsg: "Task Tracker never appeared" },
    );
    const toolBtn = await appsSection.$('button[aria-label="Task Tracker"]');
    if (
      !(await $(
        '.tool-canvas-body[data-tool-id="tool-task-tracker"]',
      ).isExisting())
    ) {
      await toolBtn.waitForClickable({ timeout: 10_000 });
      await toolBtn.click();
    }

    const canvas = await $(
      '.tool-canvas-body[data-tool-id="tool-task-tracker"]',
    );
    await canvas.waitForExist({ timeout: 15_000 });
    const titleEl = await canvas.$("#tm-new-title");
    await titleEl.waitForExist({ timeout: 10_000 });
    await titleEl.click();
    await titleEl.setValue(TASK_TITLE);
    const addBtn = await canvas.$('button[data-component-id="tm-add-btn"]');
    await addBtn.waitForClickable({ timeout: 10_000 });
    await addBtn.click();
    const approve = await $("button=Approve once");
    try {
      await approve.waitForExist({ timeout: 4_000 });
      await approve.waitForClickable({ timeout: 5_000 });
      await approve.click();
    } catch {
      // already granted
    }
    await browser.waitUntil(
      async () => (await canvas.getText()).includes(TASK_TITLE),
      { timeout: 20_000, timeoutMsg: "Task never appeared" },
    );

    // Unmount renderer before durable evolve commit (no live ack).
    await closeToolCanvas();

    await composer.click();
    await composer.setValue("Add due dates to my task tracker");
    await send.waitForClickable({ timeout: 10_000 });
    await send.click();

    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) !== null,
      { timeout: 45_000, timeoutMsg: "Evolve Apply never appeared while unmounted" },
    );
    const evolveBtn = await latestVisibleKernelApplyButton();
    if (!evolveBtn) throw new Error("Evolve Apply missing");
    await evolveBtn.waitForClickable({ timeout: 10_000 });
    await evolveBtn.click();
    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) === null,
      {
        timeout: 20_000,
        timeoutMsg: "Unmounted evolve Apply still visible",
      },
    );

    // Remount + reconnect catch-up must paint durable commit without prior ack.
    const reopen = await $(
      'section[aria-label="Personal apps"] button[aria-label="Task Tracker"]',
    );
    await reopen.waitForClickable({ timeout: 15_000 });
    await reopen.click();
    await simulateConversationReconnect();

    await browser.waitUntil(
      async () => {
        const due = await $(
          '.tool-canvas-body[data-tool-id="tool-task-tracker"] #tm-new-due',
        );
        return due.isExisting();
      },
      {
        timeout: 25_000,
        timeoutMsg: "Due date field missing after unmounted durable commit",
      },
    );
    const canvas2 = await $(
      '.tool-canvas-body[data-tool-id="tool-task-tracker"]',
    );
    const textAfter = await canvas2.getText();
    if (!textAfter.includes(TASK_TITLE)) {
      throw new Error(`Task "${TASK_TITLE}" missing after unmounted durable commit`);
    }

    const finishedAt = new Date().toISOString();
    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          status: "passed",
          journey: "reconnect-after-durable-commit",
          preservedTaskTitle: TASK_TITLE,
          dueDateFieldPresent: true,
          evolvedWhileUnmounted: true,
          startedAt,
          finishedAt,
          durationMs: Date.now() - t0,
          identity: evidenceIdentity(),
          binaryHash: evidenceBinaryHash(),
        },
        null,
        2,
      ),
    );
  });
});
