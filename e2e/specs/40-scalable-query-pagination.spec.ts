/**
 * Journey 40 — Authoritative query pagination, search, totals, and aggregates.
 *
 * Create Study Planner → seed >100 records via registered writes → verify
 * totalCount, page size, next page, search outside first page, summary accuracy.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock CORESIDE_E2E_SEED=empty \
 *     npx wdio run e2e/wdio.conf.ts --suite scalable-query-pagination
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
  invokeFromCurrentWindow,
  waitForAppReady,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/scalable-query-pagination-results.json",
);

const SEED_COUNT = 120;
const PAGE_SIZE = 25;
const SEARCH_TARGET = "Bulk Task 105";

async function openStudyPlannerCanvas() {
  const appsSection = await $('section[aria-label="Personal apps"]');
  await appsSection.waitForExist({ timeout: 20_000 });
  const canvasSel = '.tool-canvas-body[data-tool-id="tool-study-planner"]';
  const already = await $(canvasSel);
  if (!(await already.isExisting())) {
    const toolBtn = await appsSection.$('button[aria-label="Study Planner"]');
    await toolBtn.waitForClickable({ timeout: 15_000 });
    await toolBtn.click();
  }
  const canvas = await $(canvasSel);
  await canvas.waitForExist({ timeout: 15_000 });
  return canvas;
}

async function seedBulkTasks() {
  const grant = await invokeFromCurrentWindow("kernel_grant_permission", {
    applicationId: "tool-study-planner",
    permission: "local_data.write",
    scope: { scope: "application" },
  });
  if (!grant.ok) {
    throw new Error(`grant local_data.write failed: ${grant.error}`);
  }
  for (let i = 0; i < SEED_COUNT; i++) {
    const title = `Bulk Task ${String(i).padStart(3, "0")}`;
    const outcome = await invokeFromCurrentWindow(
      "kernel_invoke_registered_action",
      {
        request: {
          actionName: "local_data.write",
          applicationId: "tool-study-planner",
          surfaceId: "tool-study-planner",
          input: {
            modelId: "studyTasks",
            data: {
              title,
              status: i % 5 === 0 ? "done" : "todo",
              dueDate: i % 7 === 0 ? "2020-01-01" : "2099-06-01",
              notes: `seed-${i}`,
            },
          },
        },
      },
    );
    if (!outcome.ok) {
      throw new Error(`seed write ${i} failed: ${outcome.error}`);
    }
  }
}

describe("Journey 40 — scalable authoritative queries", () => {
  it("paginates, searches, and aggregates beyond the first page", async () => {
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

    const canvas = await openStudyPlannerCanvas();
    await seedBulkTasks();

    // Trigger refresh by toggling search clear / reopen canvas.
    const closeBtn = await $(
      '[aria-label="Close tool canvas"], [aria-label="Back to chat"]',
    );
    if (await closeBtn.isExisting()) {
      await closeBtn.click();
    }
    const reopened = await openStudyPlannerCanvas();

    await browser.waitUntil(
      async () => {
        const total = await reopened.$('[data-testid="data-table-total"]');
        if (!(await total.isExisting())) return false;
        const text = await total.getText();
        return text.includes(`${SEED_COUNT} record`);
      },
      {
        timeout: 30_000,
        timeoutMsg: `Table total never showed ${SEED_COUNT} records`,
      },
    );

    const table = await reopened.$('.tr-data-table[data-query-backed="true"]');
    await table.waitForExist({ timeout: 10_000 });
    const rows = await table.$$("tbody tr");
    expect(rows.length).toBeLessThanOrEqual(PAGE_SIZE);
    expect(rows.length).toBeGreaterThan(0);

    const nextBtn = await reopened.$(".tr-data-table-pagination-btn*=Next");
    await nextBtn.waitForClickable({ timeout: 10_000 });
    const firstPageText = await table.getText();
    await nextBtn.click();

    await browser.waitUntil(
      async () => {
        const text = await table.getText();
        return text !== firstPageText && text.includes("Bulk Task");
      },
      { timeout: 15_000, timeoutMsg: "Next page never loaded distinct rows" },
    );

    const search = await reopened.$(".tr-data-table-search");
    await search.waitForExist({ timeout: 5_000 });
    await search.click();
    await search.setValue(SEARCH_TARGET);
    await browser.keys("Enter");

    await browser.waitUntil(
      async () => (await reopened.getText()).includes(SEARCH_TARGET),
      {
        timeout: 20_000,
        timeoutMsg: `Search never found ${SEARCH_TARGET} outside first page`,
      },
    );

    await browser.waitUntil(
      async () => {
        const summaryText = (await inlineSummary.getText()).toLowerCase();
        return (
          summaryText.includes(`${SEED_COUNT} study task`) ||
          summaryText.includes(`${SEED_COUNT} study tasks`)
        );
      },
      {
        timeout: 20_000,
        timeoutMsg: "Inline summary never reflected authoritative totalCount",
      },
    );

    // Clear search and confirm total restored
    await search.click();
    await search.setValue("");
    await browser.keys("Enter");
    await browser.waitUntil(
      async () => {
        const total = await reopened.$('[data-testid="data-table-total"]');
        return (await total.getText()).includes(`${SEED_COUNT} record`);
      },
      { timeout: 15_000, timeoutMsg: "Total did not restore after clearing search" },
    );

    const identity = evidenceIdentity();
    const finishedAt = new Date().toISOString();
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          journey: 40,
          name: "scalable-query-pagination",
          status: "passed",
          startedAt,
          finishedAt,
          durationMs: Date.now() - t0,
          seedCount: SEED_COUNT,
          pageSize: PAGE_SIZE,
          searchTarget: SEARCH_TARGET,
          binaryHash: evidenceBinaryHash(),
          ...identity,
          e2eSpec: "e2e/specs/40-scalable-query-pagination.spec.ts",
        },
        null,
        2,
      ),
    );
  });
});
