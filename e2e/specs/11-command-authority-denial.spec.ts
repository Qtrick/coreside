import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import {
  denyPendingApprovalIfPresent,
  E2E_TOOL_ID,
  expectInvokeDenied,
  invokeFromCurrentWindow,
  listTauriWindows,
  openPersonalTool,
  openToolInWindow,
  requireExistingSeed,
  switchTauriWindow,
  waitForAppReady,
  waitForSeededTool,
  waitForToolCanvas,
} from "../helpers.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const evidencePath = path.resolve(
  __dirname,
  "../../reports/command-authority-results.json",
);
const repoRoot = path.resolve(__dirname, "../..");

function evidenceIdentity() {
  const commitEnv = process.env.CORESIDE_E2E_COMMIT?.trim();
  const dirtyEnv = process.env.CORESIDE_E2E_DIRTY;
  if (commitEnv && (dirtyEnv === "0" || dirtyEnv === "1")) {
    return { commit: commitEnv, dirty: dirtyEnv === "1" };
  }
  const commit = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  const porcelain = spawnSync("git", ["status", "--porcelain"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  return {
    commit: (commit.stdout || "").trim(),
    dirty: (porcelain.stdout || "").trim().length > 0,
  };
}

/** Sensitive commands that must be denied from tool-* windows before side effects. */
const SENSITIVE_DENIALS: Array<{ command: string; args?: Record<string, unknown> }> = [
  { command: "clear_conversations" },
  { command: "clear_tools" },
  { command: "create_profile_backup" },
  { command: "list_managed_backups" },
  { command: "preview_restore_backup", args: { path: "nonexistent.coreside-backup" } },
  {
    command: "restore_profile_backup",
    args: { path: "nonexistent.coreside-backup", confirm: true },
  },
  { command: "provider_key_hints" },
  { command: "store_hosted_auth_session", args: { accessToken: "x", refreshToken: "y" } },
  { command: "kernel_grant_permission", args: { applicationId: "other", permission: "local_data.write" } },
  { command: "kernel_export_package", args: { applicationId: "other" } },
  { command: "send_message", args: { conversationId: "x", content: "y" } },
  { command: "stage_chat_attachment", args: { input: { name: "x.txt", mimeType: "text/plain", dataBase64: "eA==" } } },
  { command: "cancel_chat_attachment", args: { attachmentId: "att-nonexistent" } },
];

describe("Journey 11 — tool-window command authority denial", () => {
  it("denies sensitive raw invokes and cross-tool state writes from a tool window", async () => {
    requireExistingSeed("Journey 11");

    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await waitForSeededTool();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);
    await openToolInWindow();

    const label = `tool-${E2E_TOOL_ID}`;
    await browser.waitUntil(async () => (await listTauriWindows()).includes(label), {
      timeout: 20_000,
      timeoutMsg: `Secondary window ${label} never appeared`,
    });
    await switchTauriWindow(label);

    const denials: Array<{ command: string; denied: boolean; error: string }> = [];
    for (const item of SENSITIVE_DENIALS) {
      const outcome = await expectInvokeDenied(item.command, item.args ?? {});
      denials.push({
        command: item.command,
        denied: !outcome.ok,
        error: outcome.ok ? "" : outcome.error,
      });
    }

    // Rust-side target binding: ACL allows save_tool_state, but another tool id must fail.
    const crossTool = await expectInvokeDenied("save_tool_state", {
      toolId: "tool-other-should-fail",
      state: { note: "pwn" },
    });

    // Own tool state remains readable (positive control that IPC still works).
    const ownRead = await invokeFromCurrentWindow("get_tool_state", {
      toolId: E2E_TOOL_ID,
    });
    expect(ownRead.ok).toBe(true);

    await switchTauriWindow("main");
    const stillThere = await invokeFromCurrentWindow("get_tool", {
      toolId: E2E_TOOL_ID,
    });
    expect(stillThere.ok).toBe(true);

    const identity = evidenceIdentity();
    const evidence = {
      schemaVersion: 1,
      product: "Coreside",
      generatedAt: new Date().toISOString(),
      commit: identity.commit,
      dirty: identity.dirty,
      command: "e2e:journey-11-command-authority",
      status: "generated_from_e2e",
      rawInvokeDenialTest: "passed",
      window: label,
      denials,
      crossToolStateWriteDenied: !crossTool.ok,
      ownToolStateReadable: ownRead.ok,
      seededToolStillPresent: stillThere.ok,
      publicBeta: "Not ready",
    };
    expect(denials.every((d) => d.denied)).toBe(true);
    expect(evidence.crossToolStateWriteDenied).toBe(true);
    expect(evidence.ownToolStateReadable).toBe(true);
    expect(evidence.seededToolStillPresent).toBe(true);
    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2) + "\n");
  });
});
