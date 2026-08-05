/** Shared desktop E2E helpers (WebdriverIO + Tauri embedded provider). */

import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export const E2E_TOOL_ID = "tool-e2e-notes";
export const E2E_TOOL_NAME = "E2E Notes";
export const E2E_APPROVAL_ID = "approval-e2e-1";
export const E2E_GRANT_ID = "grant-e2e-1";
export const E2E_NOTE_INPUT_ID = "e2e-note-input";

const helpersDir = path.dirname(fileURLToPath(import.meta.url));
export const E2E_REPO_ROOT = path.resolve(helpersDir, "..");

/** Commit / dirty / fingerprint identity for evidence reports (prefers orchestrator env). */
export function evidenceIdentity(): {
  commit: string;
  dirty: boolean;
  platform: string;
  arch: string;
  sourceFingerprint: string | null;
} {
  const commitEnv = process.env.CORESIDE_E2E_COMMIT?.trim();
  const dirtyEnv = process.env.CORESIDE_E2E_DIRTY;
  let commit = commitEnv || "";
  let dirty = dirtyEnv === "1";
  if (!commitEnv || (dirtyEnv !== "0" && dirtyEnv !== "1")) {
    const rev = spawnSync("git", ["rev-parse", "HEAD"], {
      cwd: E2E_REPO_ROOT,
      encoding: "utf8",
    });
    const porcelain = spawnSync("git", ["status", "--porcelain"], {
      cwd: E2E_REPO_ROOT,
      encoding: "utf8",
    });
    commit = (rev.stdout || "").trim();
    dirty = (porcelain.stdout || "").trim().length > 0;
  }
  let sourceFingerprint: string | null = null;
  try {
    const fpPath = path.join(E2E_REPO_ROOT, "reports/current-source-fingerprint.json");
    if (fs.existsSync(fpPath)) {
      const fp = JSON.parse(fs.readFileSync(fpPath, "utf8")) as {
        sourceFingerprint?: string;
      };
      sourceFingerprint =
        typeof fp.sourceFingerprint === "string" ? fp.sourceFingerprint : null;
    }
  } catch {
    sourceFingerprint = null;
  }
  return {
    commit,
    dirty,
    platform: os.platform(),
    arch: os.arch(),
    sourceFingerprint,
  };
}

/** SHA-256 of the E2E binary when resolvable; otherwise null. */
export function evidenceBinaryHash(): string | null {
  const candidates = [
    process.env.CORESIDE_E2E_BINARY?.trim(),
    path.join(E2E_REPO_ROOT, "src-tauri/target/debug/Coreside"),
    path.join(E2E_REPO_ROOT, "src-tauri/target/debug/coreside"),
  ].filter(Boolean) as string[];
  for (const candidate of candidates) {
    try {
      if (!fs.existsSync(candidate)) continue;
      const buf = fs.readFileSync(candidate);
      return createHash("sha256").update(buf).digest("hex");
    } catch {
      // try next
    }
  }
  return null;
}

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

/**
 * E2E-only: WebKit embedded WebViews used by Tauri E2E often keep
 * visibilityState=hidden and do not set navigator.webdriver. LiveWallpaper only
 * enables its automation paint path when navigator.webdriver is true — call this
 * before mounting canvas wallpapers in wallpaper pixel journeys.
 */
export async function ensureWebDriverPaintProbe() {
  if (process.env.CORESIDE_E2E !== "1") {
    throw new Error(
      "ensureWebDriverPaintProbe is E2E-only (set CORESIDE_E2E=1 via npm run e2e)",
    );
  }
  await browser.execute(() => {
    if (navigator.webdriver === true) return;
    Object.defineProperty(navigator, "webdriver", {
      get: () => true,
      configurable: true,
    });
  });
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

/** Sidebar app button (Personal apps label; legacy Personal tools fallback). */
function personalAppButtonSelector(toolName: string): string {
  return [
    `.sidebar-section[aria-label="Personal apps"] button[aria-label="${toolName}"]`,
    `.sidebar-section[aria-label="Personal tools"] button[aria-label="${toolName}"]`,
  ].join(", ");
}

export async function openPersonalTool(toolName = E2E_TOOL_NAME) {
  await waitForSeededTool();
  const btn = await $(personalAppButtonSelector(toolName));
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
      const btn = await $(personalAppButtonSelector(E2E_TOOL_NAME));
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
      const region = await $(".settings-content");
      if (!(await region.isExisting())) return false;
      const busy = await region.getAttribute("aria-busy");
      if (busy === "true") return false;
      const title = await $(".settings-group-title");
      return (await title.isExisting()) && (await title.getText()) === label;
    },
    {
      timeout: 15_000,
      timeoutMsg: `Settings category “${label}” did not finish transitioning`,
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

export async function invokeFromCurrentWindow(
  command: string,
  args: Record<string, unknown> = {},
): Promise<{ ok: true; result: unknown } | { ok: false; error: string }> {
  try {
    // Resolve (never reject) inside the page so ACL denials are evidence, not WDIO failures.
    return await browser.execute(
      (cmd, invokeArgs) => {
        const w = window as Window & {
          __TAURI__?: { core?: { invoke?: (c: string, a?: unknown) => Promise<unknown> } };
        };
        const invoke = w.__TAURI__?.core?.invoke;
        if (!invoke) {
          return Promise.resolve({
            ok: false as const,
            error: "__TAURI__.core.invoke unavailable",
          });
        }
        return invoke(cmd, invokeArgs).then(
          (result) => ({ ok: true as const, result }),
          (err: unknown) => {
            const message =
              typeof err === "string"
                ? err
                : err && typeof err === "object" && "message" in err
                  ? String((err as { message: unknown }).message)
                  : String(err);
            return { ok: false as const, error: message };
          },
        );
      },
      command,
      args,
    );
  } catch (err) {
    // Some ACL denials surface as WebDriver execute errors; treat as denial evidence.
    return {
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    };
  }
}

export async function expectInvokeDenied(
  command: string,
  args: Record<string, unknown> = {},
) {
  const outcome = await invokeFromCurrentWindow(command, args);
  expect(outcome.ok).toBe(false);
  if (!outcome.ok) {
    expect(outcome.error.length).toBeGreaterThan(0);
    // Missing IPC is not ACL denial — do not mint false-positive authority evidence.
    expect(outcome.error).not.toContain("__TAURI__.core.invoke unavailable");
    // Allowlist denial ("not allowed") or historical explicit deny both count.
    // Prefer allowlist; do not require coreside-tool-deny-sensitive wording.
    expect(outcome.error.toLowerCase()).toMatch(
      /not allowed|denied|forbidden|permission|acl|capability|cannot access/,
    );
  }
  return outcome;
}
