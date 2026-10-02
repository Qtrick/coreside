/**
 * Journey 28 — Structured form submission (submitToAgent → sealed SUI).
 *
 * Build Tic-Tac-Toe via mock ops → kernel Apply → click a cell → assert AI
 * move state patch lands (StructuredUserInput path).
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock \
 *     npx wdio run e2e/wdio.conf.ts --suite structured-form-submission
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
  "reports/structured-form-submission-results.json",
);

describe("Journey 28 — structured form submission", () => {
  it("submits a board cell via submitToAgent and applies the AI move", async () => {
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
    await composer.setValue(
      "Build me a Tic-Tac-Toe game where I play against you",
    );

    const send = await $('[aria-label="Send message"]');
    await send.waitForClickable({ timeout: 10_000 });
    await send.click();

    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) !== null,
      {
        timeout: 45_000,
        timeoutMsg: "Tic-Tac-Toe kernel Apply never appeared",
      },
    );
    const apply = await latestVisibleKernelApplyButton();
    if (!apply) throw new Error("Tic-Tac-Toe Apply missing");
    await apply.waitForClickable({ timeout: 10_000 });
    await apply.click();
    await browser.waitUntil(
      async () => (await latestVisibleKernelApplyButton()) === null,
      {
        timeout: 25_000,
        timeoutMsg: "Tic-Tac-Toe Apply still visible — decide likely failed",
      },
    );

    // Prefer durable tools list; surface may also paint inline.
    await browser.waitUntil(
      async () => {
        const listed = await invokeFromCurrentWindow("list_tools");
        if (listed.ok && Array.isArray(listed.result)) {
          const hit = (
            listed.result as Array<{ id?: string; name?: string }>
          ).some(
            (t) =>
              t.id === "tool-tictactoe" ||
              (typeof t.name === "string" && /tic/i.test(t.name)),
          );
          if (hit) return true;
        }
        if (await $('[data-component-id="ttt-c4"]').isExisting()) return true;
        const apps = await $('section[aria-label="Personal apps"]');
        if (!(await apps.isExisting())) return false;
        const labels = await apps.$$("button");
        for (const btn of labels) {
          const label = (await btn.getAttribute("aria-label")) || "";
          if (/tic/i.test(label)) return true;
        }
        return false;
      },
      {
        timeout: 25_000,
        timeoutMsg: "Tic-Tac-Toe never landed in tools after Apply",
      },
    );

    const appsSection = await $('section[aria-label="Personal apps"]');
    if (await appsSection.isExisting()) {
      let toolBtn = await appsSection.$(
        'button[aria-label="Tic-Tac-Toe vs AI"]',
      );
      if (!(await toolBtn.isExisting())) {
        const labels = await appsSection.$$("button");
        for (const btn of labels) {
          const label = (await btn.getAttribute("aria-label")) || "";
          if (/tic/i.test(label)) {
            toolBtn = btn;
            break;
          }
        }
      }
      if (
        (await toolBtn.isExisting()) &&
        !(await $(
          '.tool-canvas-body[data-tool-id="tool-tictactoe"]',
        ).isExisting()) &&
        !(await $('[data-component-id="ttt-c4"]').isExisting())
      ) {
        await toolBtn.waitForClickable({ timeout: 10_000 });
        await toolBtn.click();
      }
    }

    const center = await $('[data-component-id="ttt-c4"]');
    await center.waitForClickable({ timeout: 15_000 });
    await center.click();

    await browser.waitUntil(
      async () => {
        const root =
          (await $('.tool-canvas-body[data-tool-id="tool-tictactoe"]').isExisting())
            ? await $('.tool-canvas-body[data-tool-id="tool-tictactoe"]')
            : await $("body");
        const text = (await root.getText()).toLowerCase();
        const status = await $('[data-component-id="ttt-status"]');
        const statusText = (await status.isExisting())
          ? (await status.getText()).toLowerCase()
          : "";
        return (
          /\bo\b/.test(text) ||
          statusText.includes("your turn") ||
          statusText.includes("played") ||
          statusText.includes("turn")
        );
      },
      {
        timeout: 45_000,
        timeoutMsg: "AI move / status never updated after structured submit",
      },
    );

    const finishedAt = new Date().toISOString();
    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          status: "passed",
          journey: "structured-form-submission",
          toolId: "tool-tictactoe",
          eventName: "game.move",
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
