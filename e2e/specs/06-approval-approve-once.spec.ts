import {
  approveOnce,
  requireExistingSeed,
  waitForAppReady,
  waitForPendingApproval,
} from "../helpers.js";

describe("Journey 6 — approval approve once", () => {
  it("shows the seeded pending approval and dismisses it after Approve once", async () => {
    requireExistingSeed("Journey 6");

    await waitForAppReady();
    await waitForPendingApproval();
    await approveOnce();

    const card = await $(`#approval-e2e-1-title`);
    await expect(card).not.toBeExisting();
  });
});
