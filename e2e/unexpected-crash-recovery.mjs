#!/usr/bin/env node
/**
 * Unexpected mid-generation crash harness.
 *
 * Differs from process-relaunch-recovery.mjs: Phase A does NOT cancel. The
 * supervisor SIGKILLs the tracked E2E WebDriver holder while generation is
 * active, then Phase B asserts startup interruption → recovery banner.
 *
 * Kill safety: only PIDs listening on WebDriver :4445 whose exe basename
 * matches the E2E binary. Never kill by basename alone.
 *
 *   node e2e/unexpected-crash-recovery.mjs
 */

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const evidencePath = path.resolve(
  repoRoot,
  "reports/unexpected-crash-recovery-results.json",
);
const statePath = path.resolve(repoRoot, "reports/unexpected-crash-state.json");
const wdioConfig = path.join(__dirname, "wdio.conf.ts");
const WEBDRIVER_PORT = 4445;

function resolveE2eBinary() {
  const fromEnv = process.env.CORESIDE_E2E_BINARY?.trim();
  if (fromEnv && fs.existsSync(fromEnv)) return fromEnv;
  const candidates = [
    path.join(repoRoot, "src-tauri/target/debug/Coreside"),
    path.join(repoRoot, "src-tauri/target/debug/coreside"),
  ];
  return candidates.find((p) => fs.existsSync(p)) ?? null;
}

function resolveExecutablePath(pid) {
  if (process.platform === "darwin") {
    const out = spawnSync("ps", ["-p", String(pid), "-o", "comm="], {
      encoding: "utf8",
    });
    return (out.stdout || "").trim() || null;
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

function pathsMatchBinary(candidate, binaryPath) {
  if (!candidate) return false;
  try {
    const resolvedBinary = path.resolve(binaryPath);
    const resolvedCandidate = path.resolve(candidate);
    if (resolvedCandidate === resolvedBinary) return true;
    // darwin `ps -o comm=` returns basename only — accept that form exclusively.
    // Never match basename when candidate is a full path to a different binary
    // (e.g. packaged /Applications/.../Coreside vs target/debug/Coreside).
    if (!candidate.includes(path.sep) && path.basename(resolvedBinary) === candidate) {
      return true;
    }
    return false;
  } catch {
    return false;
  }
}

/** Only PIDs holding :4445 that match the E2E binary. */
function webDriverHolderPids(binaryPath) {
  if (process.platform === "win32") return [];
  const lsof = spawnSync(
    "lsof",
    ["-nP", `-iTCP:${WEBDRIVER_PORT}`, "-sTCP:LISTEN", "-t"],
    { encoding: "utf8" },
  );
  return (lsof.stdout || "")
    .split(/\s+/)
    .map((s) => s.trim())
    .filter(Boolean)
    .map(Number)
    .filter((n) => Number.isInteger(n) && n > 0)
    .filter((pid) => {
      if (pid === process.pid) return false;
      const exe = resolveExecutablePath(pid);
      return pathsMatchBinary(exe, binaryPath);
    });
}

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

async function waitForStateFile(timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (fs.existsSync(statePath)) {
      try {
        const state = JSON.parse(fs.readFileSync(statePath, "utf8"));
        if (state.mode === "in_flight_awaiting_sigkill") return state;
      } catch {
        /* partial write */
      }
    }
    await sleep(400);
  }
  return null;
}

const startedAt = new Date().toISOString();
const binary = resolveE2eBinary();
if (!binary) {
  const evidence = {
    harness: "unexpected-crash-recovery",
    mode: "blocked",
    status: "blocked",
    note: "E2E binary missing — run npm run e2e:build first",
    startedAt,
    finishedAt: new Date().toISOString(),
  };
  fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
  fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));
  console.error(JSON.stringify(evidence, null, 2));
  process.exit(1);
}

const dbDir = fs.mkdtempSync(path.join(os.tmpdir(), "coreside-crash-"));
const dbPath = path.join(dbDir, "coreside.db");
try {
  fs.unlinkSync(statePath);
} catch {
  /* absent ok */
}

const baseEnv = {
  ...process.env,
  CORESIDE_E2E: "1",
  CORESIDE_DB_PATH: dbPath,
  CORESIDE_E2E_BINARY: binary,
  AI_PROVIDER: process.env.AI_PROVIDER || "mock",
  CORESIDE_E2E_SEED: "existing",
  CORESIDE_UNEXPECTED_CRASH_STATE: statePath,
  NO_PROXY: "127.0.0.1,localhost",
  no_proxy: "127.0.0.1,localhost",
};
delete baseEnv.CARGO_TARGET_DIR;

console.log(`[unexpected-crash] Phase A starting binary=${binary}`);
const phaseA = spawn(
  "npx",
  ["wdio", "run", wdioConfig, "--suite", "unexpected-crash-phase-a"],
  {
    cwd: repoRoot,
    env: baseEnv,
    stdio: ["ignore", "pipe", "pipe"],
  },
);

let phaseAOut = "";
phaseA.stdout?.on("data", (d) => {
  phaseAOut += d.toString();
});
phaseA.stderr?.on("data", (d) => {
  phaseAOut += d.toString();
});

const state = await waitForStateFile(90_000);
if (!state) {
  phaseA.kill("SIGTERM");
  console.error("[unexpected-crash] Phase A never wrote in-flight state");
  console.error(phaseAOut.slice(-6000));
  process.exit(1);
}

const holders = webDriverHolderPids(binary);
if (holders.length === 0) {
  phaseA.kill("SIGTERM");
  console.error(
    "[unexpected-crash] no tracked WebDriver holder on :4445 matching E2E binary",
  );
  process.exit(1);
}

console.log(`[unexpected-crash] SIGKILL pids=${holders.join(",")}`);
for (const pid of holders) {
  try {
    process.kill(pid, "SIGKILL");
  } catch (err) {
    console.warn(`[unexpected-crash] kill ${pid} failed: ${err}`);
  }
}
try {
  phaseA.kill("SIGKILL");
} catch {
  /* already dead */
}

await sleep(1500);

console.log("[unexpected-crash] Phase B starting");
const phaseB = spawnSync(
  "npx",
  ["wdio", "run", wdioConfig, "--suite", "unexpected-crash-phase-b"],
  {
    cwd: repoRoot,
    env: baseEnv,
    encoding: "utf8",
  },
);
const passed = phaseB.status === 0;
if (!passed) {
  console.error((phaseB.stdout || "") + (phaseB.stderr || ""));
}

const evidence = {
  harness: "unexpected-crash-recovery",
  mode: "sigkill_mid_generation",
  evidenceLevel: passed
    ? "Full OS Process Crash Relaunch Verified"
    : "Native Debug Desktop E2E Verified",
  note: "Phase A left generation in flight; supervisor SIGKILLed WebDriver holder on :4445 matching E2E binary only. Not user Cancel.",
  startedAt,
  finishedAt: new Date().toISOString(),
  host: os.hostname(),
  status: passed ? "passed" : "failed",
  dbPath,
  binary,
  killedPids: holders,
  phaseBExit: phaseB.status,
  state,
};
fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
// Phase B may have written evidence already — merge status if missing.
if (!fs.existsSync(evidencePath) || !passed) {
  fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));
} else {
  try {
    const prior = JSON.parse(fs.readFileSync(evidencePath, "utf8"));
    fs.writeFileSync(
      evidencePath,
      JSON.stringify({ ...prior, supervisor: evidence }, null, 2),
    );
  } catch {
    fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));
  }
}
console.log(JSON.stringify(evidence, null, 2));

if (passed) {
  try {
    fs.rmSync(dbDir, { recursive: true, force: true });
  } catch {
    /* best-effort */
  }
}
process.exit(passed ? 0 : 1);
