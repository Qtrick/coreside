import {
  closeToolCanvas,
  denyPendingApprovalIfPresent,
  E2E_NOTE_INPUT_ID,
  E2E_TOOL_ID,
  openPersonalTool,
  requireExistingSeed,
  waitForAppReady,
  waitForSeededTool,
  waitForToolCanvas,
} from "../helpers.js";

describe("Journey 5 — generated tool open + state persist", () => {
  it("opens the seeded tool and persists text input state across close/reopen", async () => {
    requireExistingSeed("Journey 5");

    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await waitForSeededTool();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

    const input = await $(`#${E2E_NOTE_INPUT_ID}`);
    await input.waitForExist({ timeout: 10_000 });
    await input.setValue("E2E persisted note");

    await closeToolCanvas();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

    const reopened = await $(`#${E2E_NOTE_INPUT_ID}`);
    await expect(reopened).toHaveValue("E2E persisted note");
  });
});
