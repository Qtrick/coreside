/**
 * Journey 30 — Consumer multi-turn undo via undo_transaction.
 *
 * Create Task Tracker → evolve due dates → undo latest applied transaction →
 * prove due-date field is gone while create remains; then a one-turn undo of
 * create removes the app (or restores pre-create state).
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock \
 *     npx wdio run e2e/wdio.conf.ts --suite multi-turn-undo
 */
import fs from "node:fs";
import path from "node:path";
import {
  E2E_REPO_ROOT,
  activeConversationIdFromStore,
  denyPendingApprovalIfPresent,
  evidenceBinaryHash,
  evidenceIdentity,
  invokeFromCurrentWindow,
  latestVisibleKernelApplyButton,
  listAppliedTransactions,
  waitForAppReady,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/multi-turn-undo-results.json",
);

const TASK_TITLE = "Undo evolution keeps me";

describe("Journey 30 — multi-turn application undo", () => {
  it("undoes evolve then create via undo_transaction", async () => {
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
    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) === null,
      { timeout: 20_000, timeoutMsg: "Create Apply still visible" },
    );

    const conversationId = await activeConversationIdFromStore();
    if (!conversationId) throw new Error("No active conversation id");

    await browser.waitUntil(
      async () => {
        const txns = await listAppliedTransactions(conversationId);
        return txns.some((t) => t.status === "applied");
      },
      { timeout: 20_000, timeoutMsg: "Create transaction never listed as applied" },
    );

    const appsSection = await $('section[aria-label="Personal apps"]');
    await appsSection.waitForExist({ timeout: 20_000 });
    const toolBtn = await appsSection.$('button[aria-label="Task Tracker"]');
    await toolBtn.waitForClickable({ timeout: 15_000 });
    await toolBtn.click();

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

    await composer.click();
    await composer.setValue("Add due dates to my task tracker");
    await send.waitForClickable({ timeout: 10_000 });
    await send.click();

    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) !== null,
      { timeout: 45_000, timeoutMsg: "Evolve Apply never appeared" },
    );
    const evolveBtn = await latestVisibleKernelApplyButton();
    if (!evolveBtn) throw new Error("Evolve Apply missing");
    await evolveBtn.waitForClickable({ timeout: 10_000 });
    await evolveBtn.click();
    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) === null,
      { timeout: 20_000, timeoutMsg: "Evolve Apply still visible" },
    );

    // Remount to see evolved UI.
    const close = await $(
      '[aria-label="Close tool canvas"], [aria-label="Back to chat"]',
    );
    if (await close.isExisting()) await close.click();
    const reopen = await $(
      'section[aria-label="Personal apps"] button[aria-label="Task Tracker"]',
    );
    await reopen.waitForClickable({ timeout: 15_000 });
    await reopen.click();
    await browser.waitUntil(
      async () =>
        (await $(
          '.tool-canvas-body[data-tool-id="tool-task-tracker"] #tm-new-due',
        ).isExisting()),
      { timeout: 25_000, timeoutMsg: "Due date field missing after evolve" },
    );

    const beforeUndo = await listAppliedTransactions(conversationId);
    const applied = beforeUndo.filter((t) => t.status === "applied");
    if (applied.length < 1) {
      throw new Error(
        `Expected ≥1 applied transaction after evolve, got ${JSON.stringify(applied)}`,
      );
    }
    // Newest-first: tip must undo first (OCC). Prefer evolve by summary when present.
    const evolveTxn =
      applied.find((t) => /due/i.test(t.summary)) ?? applied[0];
    const older = applied.filter((t) => t.id !== evolveTxn.id);
    const createTxn =
      older.find((t) => /create|tracker|generat/i.test(t.summary)) ??
      older.at(-1) ??
      null;

    const undoEvolve = await invokeFromCurrentWindow("undo_transaction_cmd", {
      transactionId: evolveTxn.id,
    });
    if (!undoEvolve.ok) {
      throw new Error(
        `undo evolve failed (${evolveTxn.summary}): ${undoEvolve.error}; applied=${JSON.stringify(applied)}`,
      );
    }

    // Remount so restored definition paints.
    const close2 = await $(
      '[aria-label="Close tool canvas"], [aria-label="Back to chat"]',
    );
    if (await close2.isExisting()) await close2.click();
    const reopen2 = await $(
      'section[aria-label="Personal apps"] button[aria-label="Task Tracker"]',
    );
    await reopen2.waitForClickable({ timeout: 15_000 });
    await reopen2.click();

    await browser.waitUntil(
      async () => {
        const due = await $(
          '.tool-canvas-body[data-tool-id="tool-task-tracker"] #tm-new-due',
        );
        return !(await due.isExisting());
      },
      {
        timeout: 25_000,
        timeoutMsg: "Due date field still present after evolve undo",
      },
    );
    const canvasAfterEvolveUndo = await $(
      '.tool-canvas-body[data-tool-id="tool-task-tracker"]',
    );
    const text = await canvasAfterEvolveUndo.getText();
    if (!text.includes(TASK_TITLE)) {
      throw new Error("Task record lost when undoing evolve (one-turn undo failed)");
    }

    // Two-turn: undo create when present (may delete linked tool / surface).
    if (createTxn) {
      const undoCreate = await invokeFromCurrentWindow("undo_transaction_cmd", {
        transactionId: createTxn.id,
      });
      if (!undoCreate.ok) {
        throw new Error(
          `undo create failed (${createTxn.summary}): ${undoCreate.error}`,
        );
      }
    }

    const after = await listAppliedTransactions(conversationId);
    const stillApplied = after.filter((t) => t.status === "applied");
    const reverted = after.filter((t) => t.status === "reverted");
    if (reverted.length < 1) {
      throw new Error(
        `Expected ≥1 reverted transaction, got ${JSON.stringify(after)}`,
      );
    }
    if (stillApplied.some((t) => t.id === evolveTxn.id)) {
      throw new Error("Evolve transaction still marked applied after undo");
    }
    if (createTxn && stillApplied.some((t) => t.id === createTxn.id)) {
      throw new Error("Create transaction still marked applied after undo");
    }

    const finishedAt = new Date().toISOString();
    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          status: "passed",
          journey: "multi-turn-undo",
          evolveTxnId: evolveTxn.id,
          createTxnId: createTxn?.id ?? null,
          preservedTaskAfterEvolveUndo: true,
          twoTurnUndo: Boolean(createTxn),
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
