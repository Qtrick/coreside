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
import { execFileSync } from "node:child_process";
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
  "reports/scalable-query-pagination-results.json",
);

const SEED_COUNT = 120;
const PAGE_SIZE = 25;
const SEARCH_TARGET = "Bulk Task 105";
const APP_ID = "tool-study-planner";
const MODEL_ID = "studyTasks";

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

/**
 * Backend seed into the isolated E2E SQLite profile.
 * Avoids registered-action rate limits (60/min) while still verifying production
 * query/render paths after reopen.
 */
function seedBulkTasksViaSqlite() {
  const dbPath = process.env.CORESIDE_DB_PATH?.trim();
  if (!dbPath || !fs.existsSync(dbPath)) {
    throw new Error(`CORESIDE_DB_PATH missing or absent: ${dbPath ?? "(unset)"}`);
  }
  const now = new Date().toISOString();
  const stmts: string[] = ["BEGIN;"];
  for (let i = 0; i < SEED_COUNT; i++) {
    const title = `Bulk Task ${String(i).padStart(3, "0")}`;
    const status = i % 5 === 0 ? "done" : "todo";
    const dueDate = i % 7 === 0 ? "2020-01-01" : "2099-06-01";
    const data = JSON.stringify({
      title,
      status,
      dueDate,
      notes: `seed-${i}`,
    }).replace(/'/g, "''");
    const id = `rec-e2e-bulk-${String(i).padStart(4, "0")}`;
    stmts.push(
      `INSERT INTO generated_data_records (
         id, application_id, model_id, record_version, data_json, created_at, updated_at
       ) VALUES (
         '${id}', '${APP_ID}', '${MODEL_ID}', 1, '${data}', '${now}', '${now}'
       );`,
    );
  }
  stmts.push(
    `INSERT INTO generated_data_revisions (application_id, model_id, revision)
     VALUES ('${APP_ID}', '${MODEL_ID}', ${SEED_COUNT})
     ON CONFLICT(application_id, model_id) DO UPDATE SET revision = revision + ${SEED_COUNT};`,
  );
  stmts.push("COMMIT;");
  execFileSync("sqlite3", [dbPath], {
    input: stmts.join("\n"),
    encoding: "utf8",
  });
  const count = execFileSync(
    "sqlite3",
    [dbPath, `SELECT COUNT(*) FROM generated_data_records WHERE application_id='${APP_ID}' AND model_id='${MODEL_ID}';`],
    { encoding: "utf8" },
  ).trim();
  if (Number(count) < SEED_COUNT) {
    throw new Error(`sqlite seed underfilled: got ${count}, want >= ${SEED_COUNT}`);
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

    // Close any open canvas so hydration does not race the backend seed.
    const earlyClose = await $(
      '[aria-label="Close tool canvas"], [aria-label="Back to chat"]',
    );
    if (await earlyClose.isExisting()) {
      await earlyClose.click();
    }
    seedBulkTasksViaSqlite();
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

    // Canvas dashboard summary must use authoritative totals (not page length).
    await browser.waitUntil(
      async () => {
        const dash = await reopened.$('[data-component-id="sp-dash-summary"]');
        if (!(await dash.isExisting())) return false;
        const t = (await dash.getText()).toLowerCase();
        return (
          t.includes(`${SEED_COUNT} study task`) ||
          t.includes(`${SEED_COUNT} study tasks`)
        );
      },
      {
        timeout: 20_000,
        timeoutMsg: "Canvas summary never reflected authoritative totalCount",
      },
    );

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

    // One authorized UI write emits runtime-application-data-changed so the
    // inline surface catches up after the backend SQL seed.
    const notifyTitle = "Reactive notify task";
    const titleEl = await reopened.$("#sp-new-title");
    await titleEl.click();
    await titleEl.setValue(notifyTitle);
    const addBtn = await reopened.$('button[data-component-id="sp-add-btn"]');
    await addBtn.waitForClickable({ timeout: 10_000 });
    await addBtn.click();
    await approveOnceIfPresent();
    await browser.waitUntil(
      async () => (await reopened.getText()).includes(notifyTitle),
      {
        timeout: 20_000,
        timeoutMsg: "Notify write never appeared in the canvas table",
      },
    );

    const expectedInline = SEED_COUNT + 1;
    await browser.waitUntil(
      async () => {
        const summary = await $(
          '.inline-surface[data-surface-id="s-study-planner-summary"]',
        );
        if (!(await summary.isExisting())) return false;
        const t = (await summary.getText()).toLowerCase();
        return (
          t.includes(`${expectedInline} study task`) ||
          t.includes(`${expectedInline} study tasks`)
        );
      },
      {
        timeout: 25_000,
        timeoutMsg: "Inline summary never reflected authoritative totalCount",
      },
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
