import fs from "node:fs";
import path from "node:path";
import {
  E2E_REPO_ROOT,
  E2E_TOOL_ID,
  denyPendingApprovalIfPresent,
  openPersonalTool,
  requireExistingSeed,
  waitForAppReady,
  waitForToolCanvas,
} from "../helpers.js";

/**
 * Process-relaunch Phase B — NEW OS process, same isolated DB as Phase A.
 * Asserts recoverable generation survived full process exit + relaunch.
 */
const statePath =
  process.env.CORESIDE_PROCESS_RELAUNCH_STATE?.trim() ||
  path.resolve(E2E_REPO_ROOT, "reports/process-relaunch-state.json");

describe("Process relaunch Phase B — recovery after new process", () => {
  it("lists recoverable generation and mounts read-only preview", async () => {
    requireExistingSeed("process-relaunch Phase B");
    expect(fs.existsSync(statePath)).toBe(true);
    const prior = JSON.parse(fs.readFileSync(statePath, "utf8")) as {
      dbPath: string;
      recoveryVisible: boolean;
    };
    expect(prior.recoveryVisible).toBe(true);
    expect(process.env.CORESIDE_DB_PATH).toBe(prior.dbPath);

    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

    const recovery = await $('[data-testid="generation-recovery"]');
    await browser.waitUntil(
      async () => {
        await browser.execute(() => window.dispatchEvent(new Event("focus")));
        return recovery.isExisting();
      },
      {
        timeout: 25_000,
        interval: 500,
        timeoutMsg:
          "Phase B: recovery banner missing after OS process relaunch",
      },
    );

    const review = await $('button=Review Preview');
    await review.waitForClickable({ timeout: 5_000 });
    await review.click();

    const host = await $('[data-testid="recovered-preview-host"]');
    await host.waitForExist({
      timeout: 10_000,
      timeoutMsg: "Phase B: recovered preview host never mounted",
    });
    expect(await host.getAttribute("data-fidelity")).toBeDefined();
    const surfaces = await $$('[data-testid="recovered-preview-surface"]');
    expect(surfaces.length).toBeGreaterThan(0);

    const updated = {
      ...prior,
      phase: "B",
      verifiedAt: new Date().toISOString(),
      previewMounted: true,
      surfaceCount: surfaces.length,
    };
    fs.writeFileSync(statePath, JSON.stringify(updated, null, 2));
  });
});
