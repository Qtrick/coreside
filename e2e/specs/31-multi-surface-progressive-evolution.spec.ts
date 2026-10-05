/**
 * Journey 31 — Multi-surface progressive durable apply (Study Planner).
 *
 * Strongest path supported today: ApplicationPlan create with Dashboard + Tasks
 * sections on one surface → Apply → add a task → evolve with priority → remount
 * → dashboard section + priority column remain durable.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock CORESIDE_E2E_SEED=empty \
 *     npx wdio run e2e/wdio.conf.ts --suite multi-surface-progressive-evolution
 */
import fs from "node:fs";
import path from "node:path";
import {
  E2E_REPO_ROOT,
  denyPendingApprovalIfPresent,
  evidenceBinaryHash,
  evidenceIdentity,
  latestVisibleKernelApplyButton,
  waitForAppReady,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/multi-surface-progressive-evolution-results.json",
);

const TASK_TITLE = "Preserve me across multi-surface evolve";

describe("Journey 31 — multi-surface progressive evolution", () => {
  it("creates Study Planner then evolves priority while preserving sections", async () => {
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

    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) !== null,
      {
        timeout: 45_000,
        timeoutMsg: "Create Apply never appeared (Study Planner ApplicationPlan)",
      },
    );
    const createApply = await latestVisibleKernelApplyButton();
    if (!createApply) {
      throw new Error("Create Apply missing after wait");
    }
    await createApply.waitForClickable({ timeout: 10_000 });
    await createApply.click();

    const appsSection = await $('section[aria-label="Personal apps"]');
    await appsSection.waitForExist({ timeout: 20_000 });
    await browser.waitUntil(
      async () => {
        const btn = await appsSection.$('button[aria-label="Study Planner"]');
        return btn.isExisting();
      },
      {
        timeout: 20_000,
        timeoutMsg: "Study Planner never appeared in Personal apps",
      },
    );
    const toolBtn = await appsSection.$('button[aria-label="Study Planner"]');
    const canvasAlready = await $(
      '.tool-canvas-body[data-tool-id="tool-study-planner"]',
    );
    if (!(await canvasAlready.isExisting())) {
      await toolBtn.waitForClickable({ timeout: 10_000 });
      await toolBtn.click();
    }

    await browser.waitUntil(
      async () => {
        const canvas = await $(
          '.tool-canvas-body[data-tool-id="tool-study-planner"]',
        );
        return canvas.isExisting();
      },
      { timeout: 15_000, timeoutMsg: "Study Planner canvas never opened" },
    );

    const canvas = await $(
      '.tool-canvas-body[data-tool-id="tool-study-planner"]',
    );
    const textBefore = (await canvas.getText()).toLowerCase();
    if (!textBefore.includes("dashboard") || !textBefore.includes("task")) {
      throw new Error(
        "Study Planner missing Dashboard/Tasks sections after create Apply",
      );
    }

    const titleEl = await canvas.$("#sp-new-title");
    await titleEl.waitForExist({ timeout: 10_000 });
    await titleEl.click();
    await titleEl.setValue(TASK_TITLE);

    const addBtn = await canvas.$('button[data-component-id="sp-add-btn"]');
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
        timeoutMsg: "Created study task never appeared in UI",
      },
    );

    await composer.click();
    await composer.setValue("Add priority to my study planner tasks");
    await send.waitForClickable({ timeout: 10_000 });
    await send.click();

    await browser.waitUntil(
      async () => {
        const body = (await $("body").getText()).toLowerCase();
        return (
          body.includes("priority") &&
          (body.includes("updated") ||
            body.includes("preserved") ||
            body.includes("dashboard"))
        );
      },
      {
        timeout: 45_000,
        timeoutMsg: "Evolve assistant response never appeared",
      },
    );

    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) !== null,
      {
        timeout: 20_000,
        timeoutMsg: "Evolve Apply never appeared (Study Planner priority plan)",
      },
    );
    const evolveBtn = await latestVisibleKernelApplyButton();
    if (!evolveBtn) {
      throw new Error("Evolve Apply missing after wait");
    }
    await evolveBtn.waitForClickable({ timeout: 10_000 });
    await evolveBtn.click();

    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) === null,
      {
        timeout: 20_000,
        timeoutMsg: "Evolve Apply still visible — kernel decide likely failed",
      },
    );

    const close = await $(
      '[aria-label="Close tool canvas"], [aria-label="Back to chat"]',
    );
    if (await close.isExisting()) {
      await close.click();
    }
    const reopen = await $(
      'section[aria-label="Personal apps"] button[aria-label="Study Planner"]',
    );
    await reopen.waitForClickable({ timeout: 15_000 });
    await reopen.click();

    await browser.waitUntil(
      async () => {
        const body = await $(
          '.tool-canvas-body[data-tool-id="tool-study-planner"]',
        );
        if (!(await body.isExisting())) return false;
        const text = (await body.getText()).toLowerCase();
        return text.includes("priority") && text.includes("dashboard");
      },
      {
        timeout: 25_000,
        timeoutMsg:
          "Priority column / Dashboard never appeared after multi-surface evolve",
      },
    );

    const canvas2 = await $(
      '.tool-canvas-body[data-tool-id="tool-study-planner"]',
    );
    const textAfter = await canvas2.getText();
    if (!textAfter.includes(TASK_TITLE)) {
      throw new Error(
        `Existing task "${TASK_TITLE}" missing after evolution — data not preserved`,
      );
    }

    const finishedAt = new Date().toISOString();
    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          status: "passed",
          journey: 31,
          name: "multi-surface-progressive-evolution",
          protocol: "applicationPlan",
          fixture: "multi_surface_planner",
          preservedTaskTitle: TASK_TITLE,
          dashboardSectionPresent: true,
          priorityColumnPresent: true,
          startedAt,
          finishedAt,
          durationMs: Date.now() - t0,
          identity: evidenceIdentity(),
          binaryHash: evidenceBinaryHash(),
          e2eSpec: "e2e/specs/31-multi-surface-progressive-evolution.spec.ts",
        },
        null,
        2,
      ),
    );
  });
});
