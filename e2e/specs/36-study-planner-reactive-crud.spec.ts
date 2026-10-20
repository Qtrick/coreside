/**
 * Journey 36 — Study Planner multi-field CRUD + reactive summary.
 *
 * Create Study Planner → add task with due date + notes → complete → reopen →
 * edit → delete → evolve priority → verify canvas + inline summary stay in sync.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock CORESIDE_E2E_SEED=empty \
 *     npx wdio run e2e/wdio.conf.ts --suite study-planner-reactive-crud
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
  latestVisibleKernelApplyButton,
  waitForAppReady,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/study-planner-reactive-crud-results.json",
);

const TASK_TITLE = "Review Chapter 5 reactive notes";
const TASK_NOTES = "Focus on mitochondria — unicode ✓";
const TASK_DUE = "2030-06-15";
const EDITED_NOTES = "Updated notes after edit";

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

describe("Journey 36 — Study Planner reactive multi-field CRUD", () => {
  it("creates, edits, completes, deletes tasks with reactive summaries", async () => {
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

    const titleEl = await canvas.$("#sp-new-title");
    await titleEl.waitForExist({ timeout: 10_000 });
    await titleEl.click();
    await titleEl.setValue(TASK_TITLE);

    const dueEl = await canvas.$("#sp-new-due");
    if (await dueEl.isExisting()) {
      await dueEl.click();
      await dueEl.setValue(TASK_DUE);
    }

    const notesEl = await canvas.$("#sp-new-notes");
    if (await notesEl.isExisting()) {
      await notesEl.click();
      await notesEl.setValue(TASK_NOTES);
    }

    const addBtn = await canvas.$('button[data-component-id="sp-add-btn"]');
    await addBtn.waitForClickable({ timeout: 10_000 });
    await addBtn.click();
    await approveOnceIfPresent();

    await browser.waitUntil(
      async () => (await canvas.getText()).includes(TASK_TITLE),
      { timeout: 20_000, timeoutMsg: "Task title never appeared after Add" },
    );
    await browser.waitUntil(
      async () => {
        const text = await canvas.getText();
        return text.includes(TASK_DUE) && text.includes(TASK_NOTES);
      },
      {
        timeout: 15_000,
        timeoutMsg: "Due date / notes never appeared in task table",
      },
    );

    await browser.waitUntil(
      async () => {
        const summaryText = (await inlineSummary.getText()).toLowerCase();
        const canvasText = (await canvas.getText()).toLowerCase();
        return (
          (summaryText.includes("1 study task") ||
            summaryText.includes("1 remaining")) &&
          (canvasText.includes("remaining") || canvasText.includes("1"))
        );
      },
      {
        timeout: 15_000,
        timeoutMsg: "Inline / dashboard summary never reflected created task",
      },
    );

    // Select row then Mark Done
    const row = await canvas.$(`td*=${TASK_TITLE}`);
    await row.waitForExist({ timeout: 10_000 });
    await row.click();

    const doneBtn = await canvas.$('button[data-component-id="sp-done-btn"]');
    await doneBtn.waitForClickable({ timeout: 10_000 });
    await doneBtn.click();
    await approveOnceIfPresent();

    await browser.waitUntil(
      async () => {
        const summaryText = (await inlineSummary.getText()).toLowerCase();
        return (
          summaryText.includes("1 completed") ||
          summaryText.includes("0 remaining")
        );
      },
      {
        timeout: 15_000,
        timeoutMsg: "Summary never reflected completed task",
      },
    );

    const reopenBtn = await canvas.$('button[data-component-id="sp-reopen-btn"]');
    await reopenBtn.waitForClickable({ timeout: 10_000 });
    await reopenBtn.click();
    await approveOnceIfPresent();

    await browser.waitUntil(
      async () => {
        const summaryText = (await inlineSummary.getText()).toLowerCase();
        return summaryText.includes("1 remaining");
      },
      {
        timeout: 15_000,
        timeoutMsg: "Summary never reflected reopened task",
      },
    );

    const editNotes = await canvas.$("#sp-edit-notes");
    await editNotes.waitForExist({ timeout: 10_000 });
    await editNotes.click();
    await editNotes.setValue(EDITED_NOTES);
    const saveBtn = await canvas.$(
      'button[data-component-id="sp-save-edit-btn"]',
    );
    await saveBtn.waitForClickable({ timeout: 10_000 });
    await saveBtn.click();
    await approveOnceIfPresent();

    await browser.waitUntil(
      async () => (await canvas.getText()).includes(EDITED_NOTES),
      { timeout: 15_000, timeoutMsg: "Edited notes never persisted in table" },
    );

    // Evolve priority — wait for assistant text, then Apply.
    // Evolve may land in ChangeProposalCard (data-proposal-apply) or the
    // composer tool-change-preview primary button.
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
    // Prefer authoritative kernel Apply; fall back to composer tool-change-preview.
    const kernelReady = await browser
      .waitUntil(async () => (await latestVisibleKernelApplyButton()) !== null, {
        timeout: 8_000,
      })
      .then(() => true)
      .catch(() => false);
    if (kernelReady) {
      await clickKernelApplyButton(5_000);
    } else {
      const previewApply = await $(
        '.tool-change-preview .preview-actions button.btn-primary',
      );
      await previewApply.waitForExist({
        timeout: 10_000,
        timeoutMsg: "Neither kernel nor preview Apply available for evolve",
      });
      await previewApply.scrollIntoView({ block: "nearest" });
      try {
        await previewApply.waitForClickable({ timeout: 5_000 });
        await previewApply.click();
      } catch {
        await browser.execute((el) => (el as HTMLElement).click(), previewApply);
      }
    }

    await browser.waitUntil(
      async () => {
        const preview = await $(".tool-change-preview");
        if (await preview.isExisting()) {
          try {
            if (await preview.isDisplayed()) return false;
          } catch {
            /* detached */
          }
        }
        return (await latestVisibleKernelApplyButton()) === null;
      },
      {
        timeout: 25_000,
        timeoutMsg: "Evolve Apply did not clear preview / kernel proposal",
      },
    );

    const canvasAfter = await openStudyPlannerCanvas();
    await browser.waitUntil(
      async () => {
        const text = (await canvasAfter.getText()).toLowerCase();
        return text.includes("priority") && text.includes(TASK_TITLE.toLowerCase());
      },
      {
        timeout: 25_000,
        timeoutMsg: "Priority UI / original task missing after evolve",
      },
    );

    // Delete
    const rowAgain = await canvasAfter.$(`td*=${TASK_TITLE}`);
    await rowAgain.click();
    const deleteBtn = await canvasAfter.$(
      'button[data-component-id="sp-delete-btn"]',
    );
    await deleteBtn.waitForClickable({ timeout: 10_000 });
    await deleteBtn.click();
    await approveOnceIfPresent();

    await browser.waitUntil(
      async () => {
        const text = await canvasAfter.getText();
        const summaryText = (await inlineSummary.getText()).toLowerCase();
        return (
          !text.includes(TASK_TITLE) &&
          (summaryText.includes("no study tasks") ||
            summaryText.includes("0 study task"))
        );
      },
      {
        timeout: 20_000,
        timeoutMsg: "Delete did not clear task + summary",
      },
    );

    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          status: "passed",
          journey: 36,
          name: "study-planner-reactive-crud",
          taskTitle: TASK_TITLE,
          dueDate: TASK_DUE,
          notesPersisted: true,
          completionSynced: true,
          editPersisted: true,
          deleteSynced: true,
          priorityEvolvePreserved: true,
          startedAt,
          finishedAt: new Date().toISOString(),
          durationMs: Date.now() - t0,
          identity: evidenceIdentity(),
          binaryHash: evidenceBinaryHash(),
          e2eSpec: "e2e/specs/36-study-planner-reactive-crud.spec.ts",
        },
        null,
        2,
      ),
    );
  });
});
