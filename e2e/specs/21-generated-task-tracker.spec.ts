import fs from "node:fs";
import path from "node:path";
import {
  E2E_REPO_ROOT,
  clickKernelApplyButton,
  denyPendingApprovalIfPresent,
  evidenceBinaryHash,
  evidenceIdentity,
  waitForAppReady,
} from "../helpers.js";

/**
 * Journey 21 — generate Task Tracker via ApplicationPlan mock provider contract.
 *
 * Does NOT seed the final application. Uses AI_PROVIDER=mock deterministic
 * ApplicationPlan fixture for "task tracker" → validate/compile → proposal →
 * user Apply → open → CRUD via local_data. (Legacy toolChange is derived for
 * preview; ApplicationPlan is authoritative for operations.)
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock CORESIDE_E2E_SEED=empty \
 *     npx wdio run e2e/wdio.conf.ts --suite generated-task-tracker
 */
const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/generated-task-tracker-results.json",
);

const TASK_TITLE = "Finish biology homework";

describe("Journey 21 — generated Task Tracker vertical slice", () => {
  it("generates, applies, opens, and creates a durable task record", async () => {
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

    // Composer sticky owns Apply when the same proposal is also inline in chat.
    await clickKernelApplyButton();

    // Open from Personal apps sidebar (must appear after proposal/tool apply refresh).
    // Prefer exact aria-label — WebKit WDIO throws on section…button*= partial-text CSS.
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
      {
        timeout: 15_000,
        timeoutMsg: "Task Tracker canvas never opened",
      },
    );

    // Scope to live canvas — proposal previews also render tm-new-title / Add Task.
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

    // Write-risk: Approve once auto-replays the frozen call (no second Add click).
    const approve = await $("button=Approve once");
    try {
      await approve.waitForExist({ timeout: 4_000 });
      await approve.waitForClickable({ timeout: 5_000 });
      await approve.click();
    } catch {
      // No approval prompt — write permission already satisfied.
    }

    await browser.waitUntil(
      async () => {
        const text = await canvas.getText();
        return text.includes(TASK_TITLE);
      },
      {
        timeout: 20_000,
        timeoutMsg: "Created task title never appeared in UI",
      },
    );

    // Close and reopen — durable local_data record must reload via dataSource.
    const close = await $(
      '[aria-label="Close tool canvas"], [aria-label="Back to chat"]',
    );
    if (await close.isExisting()) {
      await close.click();
    }
    const reopen = await $('section[aria-label="Personal apps"] button[aria-label="Task Tracker"]');
    await reopen.waitForClickable({ timeout: 10_000 });
    await reopen.click();
    const canvasAgain = await $(
      '.tool-canvas-body[data-tool-id="tool-task-tracker"]',
    );
    await canvasAgain.waitForExist({ timeout: 15_000 });
    await browser.waitUntil(
      async () => {
        const text = await canvasAgain.getText();
        return text.includes(TASK_TITLE);
      },
      {
        timeout: 15_000,
        timeoutMsg: "Task title missing after reopen (not durable)",
      },
    );

    const identity = evidenceIdentity();
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          journey: 21,
          name: "generated-task-tracker",
          status: "passed",
          startedAt,
          finishedAt: new Date().toISOString(),
          durationMs: Date.now() - t0,
          taskTitle: TASK_TITLE,
          toolId: "tool-task-tracker",
          providerFixture: "mock task tracker (local_data)",
          seededFinalApplication: false,
          ...identity,
          binarySha256: evidenceBinaryHash(),
        },
        null,
        2,
      ),
    );
  });
});
