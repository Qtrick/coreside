import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { beforeEach, describe, expect, it } from "vitest";
import { api, isTauriRuntime, isWebPreview } from "@/lib/tauri";
import {
  MOCK_FIXTURE_MARKER,
  MOCK_ONLY_COMMANDS,
  __resetMockDb,
  __setMockAiConfigured,
  mockInvoke,
} from "@/lib/tauri/mocks";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "../../..");

describe("tauri bridge — preview vs production", () => {
  it("treats vitest/jsdom as web preview, not Tauri", () => {
    expect(isTauriRuntime()).toBe(false);
    expect(isWebPreview()).toBe(true);
  });

  it("production invoke path never imports mocks when Tauri is present", async () => {
    const invokeSrc = readFileSync(join(here, "invoke.ts"), "utf8");
    expect(invokeSrc).toMatch(/if\s*\(\s*isTauriRuntime\(\)\s*\)/);
    expect(invokeSrc).toMatch(/await import\(["']\.\/mocks["']\)/);
    // Tauri branch catches errors and rethrows — never calls mockInvoke.
    const tauriBranch = invokeSrc.match(
      /if\s*\(\s*isTauriRuntime\(\)\s*\)\s*\{([\s\S]*?)\n {2}\}/,
    )?.[1];
    expect(tauriBranch).toBeTruthy();
    expect(tauriBranch).toContain("throw new TauriCommandError");
    expect(tauriBranch).not.toContain("mockInvoke");
  });
});

describe("tauri mocks", () => {
  beforeEach(() => {
    __resetMockDb();
  });

  it("rejects unknown commands loudly", async () => {
    await expect(mockInvoke("definitely_not_a_real_command_xyz")).rejects.toThrow(
      /Unknown command/,
    );
  });

  it("resets mock state between tests", async () => {
    __setMockAiConfigured(true);
    const before = await mockInvoke<{ keyDetected: boolean; status: string }>(
      "get_ai_status",
    );
    expect(before.status).toBe("ready");

    __resetMockDb();
    const after = await mockInvoke<{ status: string }>("get_ai_status");
    expect(after.status).not.toBe("ready");
  });

  it("api wrappers reach the mock bridge outside Tauri", async () => {
    const info = await api.getAppInfo();
    expect(info.name).toBe("Coreside");
  });
});

describe("tauri API ↔ Rust command contract", () => {
  it("every api invoke maps to a registered command or explicit mock-only", () => {
    const libRs = readFileSync(join(root, "src-tauri/src/lib.rs"), "utf8");
    const registered = new Set(
      [...libRs.matchAll(/commands::([a-z0-9_]+)/g)].map((m) => m[1]),
    );

    const apiSrc = readFileSync(join(here, "api.ts"), "utf8");
    const invoked = [
      ...apiSrc.matchAll(/invoke(?:<[^>]*>)?\(\s*"([a-z0-9_]+)"/g),
    ].map((m) => m[1]);

    expect(invoked.length).toBeGreaterThan(50);

    const missing = invoked.filter(
      (cmd) => !registered.has(cmd) && !MOCK_ONLY_COMMANDS.has(cmd),
    );
    expect(missing).toEqual([]);
  });

  it("exposes a distinctive mock fixture marker for bundle checks", () => {
    expect(MOCK_FIXTURE_MARKER.length).toBeGreaterThan(10);
    expect(MOCK_FIXTURE_MARKER).toMatch(/quiz/i);
  });
});
