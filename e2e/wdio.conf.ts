import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { Options } from "@wdio/types";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const artifactsRoot = path.join(__dirname, ".artifacts");

/** WebDriver listen port used by tauri-plugin-wdio-webdriver (embedded). */
const WEBDRIVER_PORT = 4445;

function resolveAppBinary(): string {
  const fromEnv = process.env.CORESIDE_E2E_BINARY?.trim();
  if (fromEnv) return fromEnv;

  const cargoTarget = process.env.CARGO_TARGET_DIR?.trim();
  const candidates = [
    path.join(root, "src-tauri/target/debug/Coreside"),
    path.join(root, "src-tauri/target/release/Coreside"),
    path.join(root, "src-tauri/target/debug/coreside"),
    path.join(root, "src-tauri/target/release/coreside"),
  ];
  if (cargoTarget) {
    candidates.unshift(
      path.join(cargoTarget, "debug/Coreside"),
      path.join(cargoTarget, "release/Coreside"),
      path.join(cargoTarget, "debug/coreside"),
      path.join(cargoTarget, "release/coreside"),
    );
  }
  if (process.platform === "win32") {
    candidates.unshift(
      path.join(root, "src-tauri/target/debug/Coreside.exe"),
      path.join(root, "src-tauri/target/release/Coreside.exe"),
    );
  }
  const found = candidates.find((p) => fs.existsSync(p));
  if (!found) {
    throw new Error(
      "E2E binary not found. Run `npm run e2e:build` first, or set CORESIDE_E2E_BINARY.",
    );
  }
  return found;
}

function ensureIsolatedDbPath(): string {
  const existing = process.env.CORESIDE_DB_PATH?.trim();
  if (existing) return existing;
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "coreside-e2e-"));
  const dbPath = path.join(dir, "coreside.db");
  process.env.CORESIDE_DB_PATH = dbPath;
  return dbPath;
}

function resolveExecutablePath(pid: number): string | null {
  if (process.platform === "darwin") {
    const out = spawnSync("ps", ["-p", String(pid), "-o", "comm="], {
      encoding: "utf8",
    });
    const comm = (out.stdout || "").trim();
    return comm || null;
  }
  if (process.platform === "linux") {
    try {
      return fs.readlinkSync(`/proc/${pid}/exe`);
    } catch {
      return null;
    }
  }
  return null;
}

function pathsMatchBinary(candidate: string | null, binaryPath: string): boolean {
  if (!candidate) return false;
  const norm = (p: string) => path.resolve(p.replace(/\s+$/, ""));
  try {
    return (
      norm(candidate) === norm(binaryPath) ||
      path.basename(candidate) === path.basename(binaryPath)
    );
  } catch {
    return false;
  }
}

/**
 * Kill ONLY orphaned E2E WebDriver holders.
 *
 * Never terminate by matching the Coreside binary path alone — that path is
 * shared with `tauri dev` / local debug runs and would kill the user's real app.
 *
 * Safety gates (all required):
 * 1. Process is listening on the embedded WebDriver port (4445)
 * 2. Executable matches the E2E binary (basename / resolved path)
 */
function killOrphanedE2eWebDrivers(binaryPath: string) {
  if (process.platform === "win32") return;

  const lsof = spawnSync(
    "lsof",
    ["-nP", `-iTCP:${WEBDRIVER_PORT}`, "-sTCP:LISTEN", "-t"],
    { encoding: "utf8" },
  );
  const pids = (lsof.stdout || "")
    .split(/\s+/)
    .map((s) => s.trim())
    .filter(Boolean)
    .map((s) => Number(s))
    .filter((n) => Number.isInteger(n) && n > 0);

  for (const pid of pids) {
    if (pid === process.pid) continue;
    const exe = resolveExecutablePath(pid);
    if (!pathsMatchBinary(exe, binaryPath)) {
      console.warn(
        `[e2e] refusing to kill pid=${pid} on :${WEBDRIVER_PORT} (exe=${exe ?? "unknown"}; not E2E binary)`,
      );
      continue;
    }
    try {
      process.kill(pid, "SIGTERM");
      console.log(`[e2e] sent SIGTERM to orphaned E2E WebDriver pid=${pid}`);
    } catch {
      // Already exited.
    }
  }
}

function artifactDirFor(testTitle: string): string {
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const safe = testTitle.replace(/[^\w.-]+/g, "_").slice(0, 80) || "test";
  const dir = path.join(artifactsRoot, `${stamp}_${safe}`);
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

const appBinaryPath = resolveAppBinary();
const dbPath = ensureIsolatedDbPath();
const seed = process.env.CORESIDE_E2E_SEED?.trim() || "";

const appEnv: Record<string, string> = {
  CORESIDE_E2E: "1",
  CORESIDE_DB_PATH: dbPath,
  AI_PROVIDER: process.env.AI_PROVIDER || "mock",
  ...(seed ? { CORESIDE_E2E_SEED: seed } : {}),
};

export const config: Options.Testrunner = {
  runner: "local",
  tsConfigPath: path.join(__dirname, "tsconfig.json"),
  specs: ["./specs/**/*.spec.ts"],
  suites: {
    clean: ["./specs/01-clean-startup.spec.ts"],
    existing: [
      "./specs/02-existing-profile.spec.ts",
      "./specs/05-generated-tool-state.spec.ts",
      "./specs/07-multi-window-approval-race.spec.ts",
      "./specs/06-approval-approve-once.spec.ts",
      "./specs/08-grant-revoke.spec.ts",
      "./specs/09-recovery-mode.spec.ts",
      "./specs/10-secondary-window.spec.ts",
    ],
    "existing-chat": ["./specs/02-existing-profile.spec.ts"],
    "existing-tool": ["./specs/05-generated-tool-state.spec.ts"],
    "existing-approval": ["./specs/06-approval-approve-once.spec.ts"],
    "existing-approval-race": ["./specs/07-multi-window-approval-race.spec.ts"],
    "existing-grant": ["./specs/08-grant-revoke.spec.ts"],
    "existing-recovery": ["./specs/09-recovery-mode.spec.ts"],
    "existing-window": ["./specs/10-secondary-window.spec.ts"],
    main: [
      "./specs/01-clean-startup.spec.ts",
      "./specs/03-settings.spec.ts",
      "./specs/04-new-conversation-draft.spec.ts",
    ],
  },
  maxInstances: 1,
  capabilities: [
    {
      browserName: "tauri",
      "tauri:options": {
        application: appBinaryPath,
      },
    },
  ],
  logLevel: "info",
  bail: 0,
  waitforTimeout: 15_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 2,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: {
    ui: "bdd",
    timeout: 120_000,
  },
  services: [
    [
      "@wdio/tauri-service",
      {
        appBinaryPath,
        driverProvider: "embedded",
        startTimeout: 90_000,
        captureFrontendLogs: true,
        captureBackendLogs: true,
        env: appEnv,
      },
    ],
  ],
  onPrepare() {
    fs.mkdirSync(artifactsRoot, { recursive: true });
    killOrphanedE2eWebDrivers(appBinaryPath);
    process.env.CORESIDE_E2E = "1";
    process.env.CORESIDE_DB_PATH = dbPath;
    process.env.AI_PROVIDER = process.env.AI_PROVIDER || "mock";
    if (seed) process.env.CORESIDE_E2E_SEED = seed;
    else delete process.env.CORESIDE_E2E_SEED;
    console.log(`[e2e] binary=${appBinaryPath}`);
    console.log(`[e2e] CORESIDE_DB_PATH=${dbPath}`);
    console.log(`[e2e] CORESIDE_E2E_SEED=${seed || "(unset)"}`);
  },
  async afterTest(test, _context, result) {
    if (!result.error) return;
    const dir = artifactDirFor(test.title);
    try {
      const shot = path.join(dir, "failure.png");
      await browser.saveScreenshot(shot);
      const html = await browser.getPageSource();
      fs.writeFileSync(path.join(dir, "page.html"), html, "utf8");
      fs.writeFileSync(
        path.join(dir, "meta.json"),
        JSON.stringify(
          {
            title: test.title,
            file: test.file,
            error: String(result.error?.message ?? result.error),
            dbPath,
            seed: seed || null,
            binary: appBinaryPath,
          },
          null,
          2,
        ),
        "utf8",
      );
      console.error(`[e2e] failure artifacts → ${dir}`);
    } catch (err) {
      console.error(`[e2e] artifact capture failed: ${err}`);
    }
  },
  onComplete() {
    killOrphanedE2eWebDrivers(appBinaryPath);
  },
};
