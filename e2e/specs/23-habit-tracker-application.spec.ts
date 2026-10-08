import fs from "node:fs";
import path from "node:path";
import {
  E2E_REPO_ROOT,
  denyPendingApprovalIfPresent,
  evidenceBinaryHash,
  evidenceIdentity,
  clickKernelApplyButton,
  waitForAppReady,
} from "../helpers.js";

/**
 * Journey 23 — generate Habit Tracker via ApplicationPlan mock provider contract.
 *
 * Does NOT seed the final application. Uses AI_PROVIDER=mock deterministic
 * ApplicationPlan fixture for "habit tracker" → validate/compile → proposal →
 * user Apply → open → create habit via local_data.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock CORESIDE_E2E_SEED=empty \
 *     npx wdio run e2e/wdio.conf.ts --suite habit-tracker-application
 */
const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/habit-tracker-application-results.json",
);

const HABIT_TITLE = "Morning meditation";

describe("Journey 23 — generated Habit Tracker vertical slice", () => {
  it("generates, applies, opens, and creates a durable habit record", async () => {
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
    await composer.setValue("Build me a simple habit tracker");

    const send = await $('[aria-label="Send message"]');
    await send.waitForClickable({ timeout: 10_000 });
    await send.click();

    await clickKernelApplyButton();

    const appsSection = await $('section[aria-label="Personal apps"]');
    await appsSection.waitForExist({ timeout: 20_000 });
    await browser.waitUntil(
      async () => {
        const btn = await appsSection.$('button[aria-label="Habit Tracker"]');
        return btn.isExisting();
      },
      {
        timeout: 20_000,
        timeoutMsg: "Habit Tracker never appeared in Personal apps",
      },
    );
    const toolBtn = await appsSection.$('button[aria-label="Habit Tracker"]');
    const canvasAlready = await $(
      '.tool-canvas-body[data-tool-id="tool-habit-tracker"]',
    );
    if (!(await canvasAlready.isExisting())) {
      await toolBtn.waitForClickable({ timeout: 10_000 });
      await toolBtn.click();
    }

    await browser.waitUntil(
      async () => {
        const canvas = await $(
          '.tool-canvas-body[data-tool-id="tool-habit-tracker"]',
        );
        return canvas.isExisting();
      },
      {
        timeout: 15_000,
        timeoutMsg: "Habit Tracker canvas never opened",
      },
    );

    const canvas = await $(
      '.tool-canvas-body[data-tool-id="tool-habit-tracker"]',
    );
    const titleEl = await canvas.$("#hb-new-title");
    await titleEl.waitForExist({ timeout: 10_000 });
    await titleEl.click();
    await titleEl.setValue(HABIT_TITLE);

    const addBtn = await canvas.$('button[data-component-id="hb-add-btn"]');
    await addBtn.waitForClickable({ timeout: 10_000 });
    await addBtn.click();

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
        return text.includes(HABIT_TITLE);
      },
      {
        timeout: 20_000,
        timeoutMsg: "Created habit title never appeared in UI",
      },
    );

    const close = await $(
      '[aria-label="Close tool canvas"], [aria-label="Back to chat"]',
    );
    if (await close.isExisting()) {
      await close.click();
    }
    const reopen = await $(
      'section[aria-label="Personal apps"] button[aria-label="Habit Tracker"]',
    );
    await reopen.waitForClickable({ timeout: 10_000 });
    await reopen.click();
    const canvasAgain = await $(
      '.tool-canvas-body[data-tool-id="tool-habit-tracker"]',
    );
    await canvasAgain.waitForExist({ timeout: 15_000 });
    await browser.waitUntil(
      async () => {
        const text = await canvasAgain.getText();
        return text.includes(HABIT_TITLE);
      },
      {
        timeout: 15_000,
        timeoutMsg: "Habit title missing after reopen (not durable)",
      },
    );

    const identity = evidenceIdentity();
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          journey: 23,
          name: "habit-tracker-application",
          status: "passed",
          startedAt,
          finishedAt: new Date().toISOString(),
          durationMs: Date.now() - t0,
          habitTitle: HABIT_TITLE,
          toolId: "tool-habit-tracker",
          providerFixture: "mock habit tracker (local_data)",
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
