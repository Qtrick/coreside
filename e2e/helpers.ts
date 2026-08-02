/** Shared desktop E2E helpers (WebdriverIO + Tauri embedded provider). */

export const E2E_TOOL_ID = "tool-e2e-notes";
export const E2E_TOOL_NAME = "E2E Notes";
export const E2E_APPROVAL_ID = "approval-e2e-1";
export const E2E_GRANT_ID = "grant-e2e-1";
export const E2E_NOTE_INPUT_ID = "e2e-note-input";

export type TauriBrowser = WebdriverIO.Browser & {
  tauri?: {
    listWindows: () => Promise<string[]>;
    switchWindow: (label: string) => Promise<void>;
    execute: <T>(fn: () => T | Promise<T>) => Promise<T>;
  };
};

export function requireExistingSeed(suiteLabel = "this journey") {
  if (process.env.CORESIDE_E2E_SEED !== "existing") {
    throw new Error(
      `${suiteLabel} requires CORESIDE_E2E_SEED=existing (use npm run e2e / e2e:desktop)`,
    );
  }
}

export async function waitForAppReady(timeout = 30_000) {
  await browser.waitUntil(
    async () => {
      const root = await $("#root");
      return root.isExisting();
    },
    {
      timeout,
      timeoutMsg: "Coreside #root did not appear",
    },
  );
  // Shell + AI status hydrate after first paint.
  await browser.waitUntil(
    async () => {
      const sidebar = await $('[aria-label="New chat"]');
      return sidebar.isExisting();
    },
    {
      timeout,
      timeoutMsg: "Coreside sidebar did not become ready",
    },
  );
  // Wait until bootstrap finished loading conversations (empty hint OR chat rows).
  await browser.waitUntil(
    async () => {
      return browser.execute(() => {
        const empty = document.querySelector(".sidebar-empty-hint");
        const chats = document.querySelectorAll(
          '.sidebar-section[aria-label="Recent chats"] .sidebar-nav-btn',
        );
        return Boolean(empty) || chats.length > 0;
      });
    },
    {
      timeout,
      timeoutMsg: "Coreside chat list never hydrated",
    },
  );
}

export async function recentChatTitles(): Promise<string[]> {
  return browser.execute(() =>
    Array.from(
      document.querySelectorAll(
        '.sidebar-section[aria-label="Recent chats"] .sidebar-nav-btn',
      ),
    )
      .map((el) => el.getAttribute("aria-label") || "")
      .filter(Boolean),
  );
}

export async function openSettings() {
  const settingsBtn = await $(".sidebar-settings-btn[aria-label='Settings']");
  await settingsBtn.waitForClickable({ timeout: 15_000 });
  await settingsBtn.click();
  await $("h1=Settings").waitForExist({ timeout: 10_000 });
}

export async function closeSettings() {
  const back = await $("button=Back to chat");
  await back.waitForClickable({ timeout: 10_000 });
  await back.click();
  await browser.waitUntil(
    async () => !(await $("h1=Settings").isExisting()),
    {
      timeout: 10_000,
      timeoutMsg: "Settings panel did not close",
    },
  );
}

export async function openPersonalTool(toolName = E2E_TOOL_NAME) {
  await waitForSeededTool();
  const btn = await $(
    `.sidebar-section[aria-label="Personal tools"] button[aria-label="${toolName}"]`,
  );
  await btn.waitForClickable({ timeout: 15_000 });
  await btn.click();
}

export async function waitForToolCanvas(toolId = E2E_TOOL_ID) {
  const body = await $(`.tool-canvas-body[data-tool-id="${toolId}"]`);
  await body.waitForExist({ timeout: 15_000 });
  return body;
}

export async function closeToolCanvas() {
  const close = await $(
    '[aria-label="Close tool canvas"], [aria-label="Back to chat"]',
  );
  await close.waitForClickable({ timeout: 10_000 });
  await close.click();
  await browser.waitUntil(
    async () => !(await $(".tool-canvas-body").isExisting()),
    {
      timeout: 10_000,
      timeoutMsg: "Tool canvas did not close",
    },
  );
}

export async function openToolInWindow() {
  const open = await $('[aria-label="Open tool in new window"]');
  await open.waitForClickable({ timeout: 10_000 });
  await open.click();
}

export async function listTauriWindows(): Promise<string[]> {
  const tauri = (browser as TauriBrowser).tauri;
  if (!tauri?.listWindows) return [];
  return tauri.listWindows();
}

export async function switchTauriWindow(label: string) {
  const tauri = (browser as TauriBrowser).tauri;
  if (!tauri?.switchWindow) {
    throw new Error("browser.tauri.switchWindow is unavailable in this build");
  }
  await tauri.switchWindow(label);
}

export async function waitForPendingApproval(timeout = 20_000) {
  const card = await $(`#approval-${E2E_APPROVAL_ID}-title`);
  await card.waitForExist({ timeout, timeoutMsg: "Pending approval card never appeared" });
  return card;
}

export async function approveOnce() {
  const approve = await $("button=Approve once");
  await approve.waitForClickable({ timeout: 10_000 });
  await approve.click();
  await browser.waitUntil(
    async () => !(await $(`#approval-${E2E_APPROVAL_ID}-title`).isExisting()),
    {
      timeout: 15_000,
      timeoutMsg: "Approval card did not dismiss after approve",
    },
  );
}

export async function waitForSeededTool(timeout = 20_000) {
  await browser.waitUntil(
    async () => {
      const btn = await $(
        `.sidebar-section[aria-label="Personal tools"] button[aria-label="${E2E_TOOL_NAME}"]`,
      );
      return btn.isExisting();
    },
    {
      timeout,
      timeoutMsg: `Seeded tool “${E2E_TOOL_NAME}” never appeared in the sidebar`,
    },
  );
}

/** Deny a seeded pending approval so tool/settings journeys can interact with the shell. */
export async function denyPendingApprovalIfPresent(timeout = 8_000) {
  const title = await $(`#approval-${E2E_APPROVAL_ID}-title`);
  const appeared = await title
    .waitForExist({ timeout, reverse: false })
    .then(() => true)
    .catch(() => false);
  if (!appeared) return;
  const deny = await $("button=Deny");
  await deny.waitForClickable({ timeout: 10_000 });
  await deny.click();
  await browser.waitUntil(async () => !(await title.isExisting()), {
    timeout: 15_000,
    timeoutMsg: "Approval card did not dismiss after deny",
  });
}

/** Category nav label required before a settings h3/h4 is mounted (Settings IA). */
const SETTINGS_HEADING_CATEGORY: Record<string, string> = {
  Recovery: "Advanced",
  "App permissions": "Advanced",
};

/** Click a Settings sidebar category by its visible label. */
export async function openSettingsCategory(label: string) {
  await browser.execute((catLabel) => {
    const items = Array.from(
      document.querySelectorAll<HTMLButtonElement>(".settings-nav-item"),
    );
    const match = items.find((el) => {
      const lab = el.querySelector(".settings-nav-item-label");
      return lab?.textContent?.trim() === catLabel;
    });
    match?.click();
  }, label);
  await browser.waitUntil(
    async () => {
      const title = await $(".settings-group-title");
      return title.isExisting() && (await title.getText()) === label;
    },
    {
      timeout: 10_000,
      timeoutMsg: `Settings category “${label}” did not become active`,
    },
  );
}

export async function scrollSettingsToHeading(heading: string) {
  const category = SETTINGS_HEADING_CATEGORY[heading];
  if (category) {
    await openSettingsCategory(category);
  }
  await browser.waitUntil(
    async () =>
      browser.execute((text) => {
        return Array.from(document.querySelectorAll("h3, h4")).some(
          (el) => el.textContent?.trim() === text,
        );
      }, heading),
    {
      timeout: 10_000,
      timeoutMsg: `Settings heading “${heading}” not found (open the owning category first)`,
    },
  );
  await browser.execute((text) => {
    const headings = Array.from(document.querySelectorAll("h3, h4"));
    const match = headings.find((el) => el.textContent?.trim() === text);
    match?.scrollIntoView({ block: "center" });
  }, heading);
}
