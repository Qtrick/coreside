import { recentChatTitles, waitForAppReady } from "../helpers.js";

describe("Journey 2 — existing-profile startup", () => {
  it("shows the seeded conversation from CORESIDE_E2E_SEED=existing", async () => {
    if (process.env.CORESIDE_E2E_SEED !== "existing") {
      throw new Error(
        "Journey 2 requires CORESIDE_E2E_SEED=existing (use npm run e2e / e2e:desktop)",
      );
    }

    await waitForAppReady();

    await browser.waitUntil(
      async () => (await recentChatTitles()).includes("Biology notes"),
      {
        timeout: 20_000,
        timeoutMsg: "Seeded conversation “Biology notes” never appeared",
      },
    );

    const titles = await recentChatTitles();
    expect(titles).toContain("Biology notes");
  });
});
