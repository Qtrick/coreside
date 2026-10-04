/**
 * Journey 26 — Multi-window reconnect / mount isolation.
 *
 * Window A (main): open seeded tool + establish canvas mount.
 * Window B (secondary tool window): same application mount.
 * A: simulate conversation reconnect (visibility/focus catch-up).
 * B: mutate local note input.
 * A: reconnect again and verify main canvas still healthy; B still open;
 * closing A canvas must not destroy B's native window.
 *
 * Run alone:
 *   CORESIDE_E2E=1 CORESIDE_E2E_SEED=existing \
 *     npx wdio run e2e/wdio.conf.ts --suite multi-window-reconnect
 */
import fs from "node:fs";
import path from "node:path";
import {
  E2E_NOTE_INPUT_ID,
  E2E_REPO_ROOT,
  E2E_TOOL_ID,
  closeToolCanvas,
  denyPendingApprovalIfPresent,
  evidenceBinaryHash,
  evidenceIdentity,
  listTauriWindows,
  openPersonalTool,
  openToolInWindow,
  requireExistingSeed,
  simulateConversationReconnect,
  switchTauriWindow,
  waitForAppReady,
  waitForSeededTool,
  waitForToolCanvas,
} from "../helpers.js";

const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/multi-window-reconnect-results.json",
);

const NOTE_TEXT = `mw-reconnect-${Date.now()}`;

describe("Journey 26 — multi-window reconnect", () => {
  it("keeps secondary mount healthy across main reconnect and canvas close", async () => {
    requireExistingSeed("Journey 26");
    const startedAt = new Date().toISOString();
    const t0 = Date.now();

    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await waitForSeededTool();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);
    await openToolInWindow();

    const toolLabel = `tool-${E2E_TOOL_ID}`;
    await browser.waitUntil(
      async () => (await listTauriWindows()).includes(toolLabel),
      {
        timeout: 20_000,
        timeoutMsg: `Secondary window ${toolLabel} never appeared`,
      },
    );

    // A disconnect / reconnect while B remains mounted.
    await switchTauriWindow("main");
    await simulateConversationReconnect();
    await waitForToolCanvas(E2E_TOOL_ID);

    await switchTauriWindow(toolLabel);
    await browser.waitUntil(
      async () =>
        browser.execute((toolId) => {
          return Boolean(
            document.querySelector(`.tool-canvas-body[data-tool-id="${toolId}"]`) ||
              document.querySelector(`[data-tool-id="${toolId}"]`),
          );
        }, E2E_TOOL_ID),
      {
        timeout: 15_000,
        timeoutMsg: "Tool UI missing in secondary window after main reconnect",
      },
    );

    let noteMutated = false;
    const noteInB = await $(`#${E2E_NOTE_INPUT_ID}`);
    if (await noteInB.isExisting()) {
      await noteInB.waitForClickable({ timeout: 10_000 });
      await noteInB.click();
      await noteInB.setValue(NOTE_TEXT);
      noteMutated = true;
    }

    await switchTauriWindow("main");
    await simulateConversationReconnect();
    await waitForToolCanvas(E2E_TOOL_ID);

    // Closing main canvas must not destroy the secondary native window.
    await closeToolCanvas();
    expect(await listTauriWindows()).toContain(toolLabel);
    expect(await listTauriWindows()).toContain("main");

    await switchTauriWindow(toolLabel);
    const secondaryStillLive = await browser.execute((toolId) => {
      return Boolean(
        document.querySelector(`.tool-canvas-body[data-tool-id="${toolId}"]`) ||
          document.querySelector(`[data-tool-id="${toolId}"]`) ||
          document.querySelector(".tool-window"),
      );
    }, E2E_TOOL_ID);
    expect(secondaryStillLive).toBe(true);

    if (noteMutated) {
      const noteAgain = await $(`#${E2E_NOTE_INPUT_ID}`);
      await noteAgain.waitForExist({ timeout: 10_000 });
      const value = await noteAgain.getValue();
      expect(value).toContain(NOTE_TEXT);
    }

    await switchTauriWindow("main");

    const identity = evidenceIdentity();
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          journey: 26,
          name: "multi-window-reconnect",
          status: "passed",
          startedAt,
          finishedAt: new Date().toISOString(),
          durationMs: Date.now() - t0,
          toolId: E2E_TOOL_ID,
          secondaryLabel: toolLabel,
          noteMutated,
          ...identity,
          binarySha256: evidenceBinaryHash(),
        },
        null,
        2,
      ),
    );
  });
});
