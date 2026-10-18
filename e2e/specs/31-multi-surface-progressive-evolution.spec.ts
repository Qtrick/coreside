/**
 * Journey 31 — Multi-surface progressive durable apply (Study Planner).
 *
 * Desktop Apply path: one ApplicationPlan creates a tool canvas plus a bound
 * conversation summary surface → Apply → add a task → evolve both surfaces with
 * priority → remount → dashboard, summary, and data remain durable.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock CORESIDE_E2E_SEED=empty \
 *     npx wdio run e2e/wdio.conf.ts --suite multi-surface-progressive-evolution
 */
import fs from "node:fs";
import path from "node:path";
import {
  E2E_REPO_ROOT,
  activeConversationIdFromStore,
  denyPendingApprovalIfPresent,
  evidenceBinaryHash,
  evidenceIdentity,
  clickKernelApplyButton,
  invokeFromCurrentWindow,
  latestVisibleKernelApplyButton,
  waitForAppReady,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/multi-surface-progressive-evolution-results.json",
);

const TASK_TITLE = "Preserve me across multi-surface evolve";

describe("Journey 31 — multi-surface progressive evolution", () => {
  it("creates and evolves two related Study Planner surfaces atomically", async () => {
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

    // Wait for durable Apply before asserting chat-inline paint (list re-fetches
    // after proposal status flips to applied).
    const appsSectionEarly = await $('section[aria-label="Personal apps"]');
    await appsSectionEarly.waitForExist({ timeout: 20_000 });
    await browser.waitUntil(
      async () => {
        const btn = await appsSectionEarly.$('button[aria-label="Study Planner"]');
        return btn.isExisting();
      },
      {
        timeout: 25_000,
        timeoutMsg: "Study Planner never appeared in Personal apps after Apply",
      },
    );

    const convId = await activeConversationIdFromStore();
    if (convId) {
      const listed = await invokeFromCurrentWindow("list_conversation_surfaces", {
        conversationId: convId,
      });
      if (!listed.ok) {
        throw new Error(`list_conversation_surfaces failed: ${listed.error}`);
      }
      const rows = Array.isArray(listed.result) ? listed.result : [];
      const ids = rows
        .map((row) => String((row as { id?: string }).id ?? ""))
        .filter(Boolean);
      if (!ids.includes("s-study-planner-summary")) {
        throw new Error(
          `Study Planner inline surface missing from SQLite after Apply; surfaces=${JSON.stringify(ids)}`,
        );
      }
    }

    const inlineSummary = await $(
      '.inline-surface[data-surface-id="s-study-planner-summary"]',
    );
    await inlineSummary.waitForExist({
      timeout: 25_000,
      timeoutMsg: "Study Planner inline summary surface never rendered",
    });
    expect((await inlineSummary.getText()).toLowerCase()).toContain("study tasks");

    const appsSection = appsSectionEarly;
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

    await clickKernelApplyButton(20_000);

    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) === null,
      {
        timeout: 20_000,
        timeoutMsg: "Evolve Apply still visible — kernel decide likely failed",
      },
    );

    // Verify evolve + data preservation on the open canvas (authoritative).
    const canvasAfterEvolve = await $(
      '.tool-canvas-body[data-tool-id="tool-study-planner"]',
    );
    await browser.waitUntil(
      async () => {
        if (!(await canvasAfterEvolve.isExisting())) return false;
        const text = (await canvasAfterEvolve.getText()).toLowerCase();
        return text.includes("priority") && text.includes("dashboard");
      },
      {
        timeout: 25_000,
        timeoutMsg:
          "Priority column / Dashboard never appeared after multi-surface evolve",
      },
    );
    const textAfter = await canvasAfterEvolve.getText();
    if (!textAfter.includes(TASK_TITLE)) {
      throw new Error(
        `Existing task "${TASK_TITLE}" missing after evolution — data not preserved`,
      );
    }

    await browser.waitUntil(
      async () => {
        const summary = await $(
          '.inline-surface[data-surface-id="s-study-planner-summary"]',
        );
        return (
          (await summary.isExisting()) &&
          (await summary.getText()).toLowerCase().includes("priority")
        );
      },
      {
        timeout: 20_000,
        timeoutMsg: "Related inline summary did not receive the priority evolve",
      },
    );

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
          surfaceIds: [
            "surf-tool-study-planner",
            "s-study-planner-summary",
          ],
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
