import { recentChatTitles, waitForAppReady } from "../helpers.js";

describe("Journey 1 — clean startup", () => {
  it("boots against an empty isolated database", async () => {
    await waitForAppReady();

    const newChat = await $('[aria-label="New chat"]');
    await expect(newChat).toBeDisplayed();

    const emptyHint = await $(".sidebar-empty-hint");
    await emptyHint.waitForExist({ timeout: 15_000 });
    await expect(emptyHint).toBeDisplayed();

    const titles = await recentChatTitles();
    expect(titles).toEqual([]);
  });
});
