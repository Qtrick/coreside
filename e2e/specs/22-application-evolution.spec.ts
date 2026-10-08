/**
 * Journey 22 — ApplicationPlan evolution (due dates).
 *
 * Uses the production mock ApplicationPlan contract. Flow: create Task Tracker
 * via plan → Apply → create a task → ask to add due dates → evolve plan Apply →
 * remount → existing task remains → #tm-new-due appears.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock CORESIDE_E2E_SEED=empty \
 *     npx wdio run e2e/wdio.conf.ts --suite application-evolution
 */
import fs from "node:fs";
import path from "node:path";
import {
  E2E_REPO_ROOT,
  denyPendingApprovalIfPresent,
  evidenceBinaryHash,
  evidenceIdentity,
  clickKernelApplyButton,
  latestVisibleKernelApplyButton,
  waitForAppReady,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/application-evolution-results.json",
);

const TASK_TITLE = "Preserve me across evolution";

describe("Journey 22 — ApplicationPlan evolution (due dates)", () => {
  it("evolves Task Tracker with due dates while preserving records", async () => {
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

    await clickKernelApplyButton();

    const appsSection = await $('section[aria-label="Personal apps"]');
    await appsSection.waitForExist({ timeout: 20_000 });
    await browser.waitUntil(
      async () => {
        const btn = await appsSection.$('button[aria-label="Task Tracker"]');
        return btn.isExisting();
      },
      {
        timeout: 20_000,
        timeoutMsg: "Task Tracker never appeared in Personal apps",
      },
    );
    const toolBtn = await appsSection.$('button[aria-label="Task Tracker"]');
    const canvasAlready = await $(
      '.tool-canvas-body[data-tool-id="tool-task-tracker"]',
    );
    if (!(await canvasAlready.isExisting())) {
      await toolBtn.waitForClickable({ timeout: 10_000 });
      await toolBtn.click();
    }

    await browser.waitUntil(
      async () => {
        const canvas = await $(
          '.tool-canvas-body[data-tool-id="tool-task-tracker"]',
        );
        return canvas.isExisting();
      },
      { timeout: 15_000, timeoutMsg: "Task Tracker canvas never opened" },
    );

    const canvas = await $(
      '.tool-canvas-body[data-tool-id="tool-task-tracker"]',
    );
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
      {
        timeout: 20_000,
        timeoutMsg: "Created task title never appeared in UI",
      },
    );

    await composer.click();
    await composer.setValue("Add due dates to my task tracker");
    await send.waitForClickable({ timeout: 10_000 });
    await send.click();

    await browser.waitUntil(
      async () => {
        const body = (await $("body").getText()).toLowerCase();
        return (
          body.includes("due date") &&
          (body.includes("updated") ||
            body.includes("optional due") ||
            body.includes("preserved"))
        );
      },
      {
        timeout: 45_000,
        timeoutMsg: "Evolve assistant response never appeared",
      },
    );

    await clickKernelApplyButton(20_000);

    // Wait for kernel proposal to leave pending before remounting.
    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) === null,
      {
        timeout: 20_000,
        timeoutMsg: "Evolve Apply still visible — kernel decide likely failed",
      },
    );

    // Remount so evolved definition renders (open canvas may keep prior tree).
    const close = await $(
      '[aria-label="Close tool canvas"], [aria-label="Back to chat"]',
    );
    if (await close.isExisting()) {
      await close.click();
    }
    const reopen = await $(
      'section[aria-label="Personal apps"] button[aria-label="Task Tracker"]',
    );
    await reopen.waitForClickable({ timeout: 15_000 });
    await reopen.click();

    await browser.waitUntil(
      async () => {
        const due = await $(
          '.tool-canvas-body[data-tool-id="tool-task-tracker"] #tm-new-due',
        );
        return due.isExisting();
      },
      {
        timeout: 25_000,
        timeoutMsg: "Due Date input (#tm-new-due) never appeared after evolution",
      },
    );

    const canvas2 = await $(
      '.tool-canvas-body[data-tool-id="tool-task-tracker"]',
    );
    const textAfter = await canvas2.getText();
    if (!textAfter.includes(TASK_TITLE)) {
      throw new Error(
        `Existing task "${TASK_TITLE}" missing after evolution — data not preserved`,
      );
    }
    if (!textAfter.toLowerCase().includes("due")) {
      throw new Error("Due date column/header missing after evolution");
    }

    const finishedAt = new Date().toISOString();
    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          status: "passed",
          journey: "application-evolution",
          protocol: "applicationPlan",
          seededFinalApplication: false,
          preservedTaskTitle: TASK_TITLE,
          dueDateFieldPresent: true,
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
