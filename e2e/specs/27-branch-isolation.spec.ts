/**
 * Journey 27 — Branch and diverge (record + revision isolation).
 *
 * Create Task Tracker → seed a task → fork conversation → mutate branch →
 * prove source chat/app still has original records only.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock \
 *     npx wdio run e2e/wdio.conf.ts --suite branch-isolation
 */
import fs from "node:fs";
import path from "node:path";
import {
  E2E_REPO_ROOT,
  denyPendingApprovalIfPresent,
  evidenceBinaryHash,
  evidenceIdentity,
  invokeFromCurrentWindow,
  latestVisibleKernelApplyButton,
  waitForAppReady,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/branch-isolation-results.json",
);

const SOURCE_TASK = "Source-only seed task";
const BRANCH_TASK = "Branch experimental task";

describe("Journey 27 — branch and diverge", () => {
  it("keeps source records isolated after branch mutation", async () => {
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
    await titleEl.setValue(SOURCE_TASK);
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
      async () => (await canvas.getText()).includes(SOURCE_TASK),
      { timeout: 20_000, timeoutMsg: "Source task never appeared" },
    );

    // Close canvas so chat message actions are reachable.
    const closeCanvas = await $(
      '[aria-label="Close tool canvas"], [aria-label="Back to chat"]',
    );
    if (await closeCanvas.isExisting()) {
      await closeCanvas.click();
    }

    const userBubbles = await $$('article[aria-label="Your message"]');
    if (userBubbles.length === 0) {
      throw new Error("No user messages to fork from");
    }
    const lastBubble = userBubbles[userBubbles.length - 1];
    await lastBubble.scrollIntoView();
    // WebKit WebDriver does not reliably synthesize CSS :hover for opacity:0
    // message actions — trigger the React onClick via DOM click instead.
    const forkClicked = await browser.execute(() => {
      const buttons = Array.from(
        document.querySelectorAll<HTMLButtonElement>(
          '[aria-label="Fork conversation here"]',
        ),
      );
      const btn = buttons[buttons.length - 1];
      if (!btn) return false;
      btn.click();
      return true;
    });
    if (!forkClicked) {
      throw new Error("Fork conversation button not found in DOM");
    }

    await browser.waitUntil(
      async () => {
        const listed = await invokeFromCurrentWindow("list_conversations");
        return (
          listed.ok &&
          Array.isArray(listed.result) &&
          listed.result.length >= 2
        );
      },
      {
        timeout: 20_000,
        timeoutMsg: "Fork did not create a second conversation",
      },
    );
    await browser.waitUntil(
      async () => {
        const chats = await $$(
          '.sidebar-section[aria-label="Recent chats"] .sidebar-nav-btn',
        );
        return chats.length >= 2;
      },
      {
        timeout: 20_000,
        timeoutMsg: "Forked conversation never appeared in sidebar",
      },
    );

    // Open the forked app (named "Task Tracker (branch)"), not the shared source tool.
    const branchApps = await $('section[aria-label="Personal apps"]');
    await browser.waitUntil(
      async () => {
        const labels = await branchApps.$$("button");
        for (const btn of labels) {
          const label = (await btn.getAttribute("aria-label")) || "";
          if (/task tracker.*branch/i.test(label)) return true;
        }
        // Force a tools refresh via list_tools + sidebar may lag; also accept
        // any second Task Tracker button if refreshTools already ran.
        const listed = await invokeFromCurrentWindow("list_tools");
        return (
          listed.ok &&
          Array.isArray(listed.result) &&
          (
            listed.result as Array<{ id?: string; name?: string }>
          ).some(
            (t) =>
              typeof t.id === "string" &&
              t.id.startsWith("app-branch-") &&
              typeof t.name === "string" &&
              /task tracker/i.test(t.name),
          )
        );
      },
      {
        timeout: 20_000,
        timeoutMsg: "Forked Task Tracker tool never appeared",
      },
    );
    let branchTool = await branchApps.$(
      'button[aria-label="Task Tracker (branch)"]',
    );
    if (!(await branchTool.isExisting())) {
      const labels = await branchApps.$$("button");
      for (const btn of labels) {
        const label = (await btn.getAttribute("aria-label")) || "";
        if (/task tracker.*branch/i.test(label)) {
          branchTool = btn;
          break;
        }
      }
    }
    if (!(await branchTool.isExisting())) {
      // Open by durable tool id via list_tools if sidebar label lagging.
      const listed = await invokeFromCurrentWindow("list_tools");
      const forked = (
        (listed.ok && Array.isArray(listed.result) ? listed.result : []) as Array<{
          id?: string;
          name?: string;
        }>
      ).find(
        (t) =>
          typeof t.id === "string" &&
          t.id.startsWith("app-branch-") &&
          typeof t.name === "string" &&
          /task tracker/i.test(t.name),
      );
      if (!forked?.id) {
        throw new Error(
          `Forked Task Tracker missing in list_tools: ${JSON.stringify(listed.result)}`,
        );
      }
      // Click any apps button and then force canvas by selecting via DOM if needed.
      throw new Error(
        `Forked tool ${forked.id} exists but sidebar button missing — refreshTools gap`,
      );
    }
    await browser.execute(() => {
      const btn = document.querySelector<HTMLButtonElement>(
        'button[aria-label="Task Tracker (branch)"]',
      );
      btn?.click();
    });

    const branchCanvas = await $('.tool-canvas-body[data-tool-id^="app-branch-"]');
    await branchCanvas.waitForExist({ timeout: 20_000 });
    await browser.waitUntil(
      async () => (await branchCanvas.$("#tm-new-title").isExisting()),
      { timeout: 20_000, timeoutMsg: "Branch Task Tracker canvas missing" },
    );
    const branchTitle = await branchCanvas.$("#tm-new-title");
    await branchTitle.click();
    await branchTitle.setValue(BRANCH_TASK);
    const branchAdd = await branchCanvas.$(
      'button[data-component-id="tm-add-btn"]',
    );
    await branchAdd.waitForClickable({ timeout: 10_000 });
    await branchAdd.click();
    const approve2 = await $("button=Approve once");
    try {
      await approve2.waitForExist({ timeout: 4_000 });
      await approve2.waitForClickable({ timeout: 5_000 });
      await approve2.click();
    } catch {
      // already granted
    }
    await browser.waitUntil(
      async () => (await branchCanvas.getText()).includes(BRANCH_TASK),
      { timeout: 20_000, timeoutMsg: "Branch task never appeared" },
    );

    // Switch back to the older source conversation (second chat button).
    const chatBtns = await $$(
      '.sidebar-section[aria-label="Recent chats"] .sidebar-nav-btn',
    );
    // Active is usually first; click the other one.
    let switched = false;
    for (const btn of chatBtns) {
      const pressed = await btn.getAttribute("aria-current");
      const selected = await btn.getAttribute("aria-pressed");
      const cls = (await btn.getAttribute("class")) || "";
      if (pressed === "true" || selected === "true" || cls.includes("active")) {
        continue;
      }
      await btn.click();
      switched = true;
      break;
    }
    if (!switched && chatBtns.length >= 2) {
      await chatBtns[chatBtns.length - 1].click();
    }

    const sourceTool = await $(
      'section[aria-label="Personal apps"] button[aria-label="Task Tracker"]',
    );
    await sourceTool.waitForClickable({ timeout: 15_000 });
    await sourceTool.click();
    const sourceCanvas = await $(
      '.tool-canvas-body[data-tool-id="tool-task-tracker"]',
    );
    await sourceCanvas.waitForExist({ timeout: 15_000 });
    await browser.waitUntil(
      async () => (await sourceCanvas.getText()).includes(SOURCE_TASK),
      { timeout: 20_000, timeoutMsg: "Source task missing after branch diverge" },
    );
    const sourceText = await sourceCanvas.getText();
    if (sourceText.includes(BRANCH_TASK)) {
      throw new Error("Branch task leaked into source conversation surface");
    }

    const finishedAt = new Date().toISOString();
    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          status: "passed",
          journey: "branch-isolation",
          sourceTask: SOURCE_TASK,
          branchTask: BRANCH_TASK,
          sourceIsolated: true,
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
