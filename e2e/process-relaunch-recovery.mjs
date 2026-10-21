#!/usr/bin/env node
/**
 * True OS process-relaunch harness for generation recovery durability.
 *
 * Two separate WDIO sessions (two Coreside E2E process lifetimes) share one
 * isolated CORESIDE_DB_PATH. That is Full OS Process Relaunch Verified when
 * both phases exit 0 — not the same as SQLite reopen alone.
 *
 * Phase A (suite: process-relaunch-phase-a):
 *   Launch E2E app → cancel progressive hold → assert recovery → write
 *   reports/process-relaunch-state.json → exit (app process ends).
 *
 * Phase B (suite: process-relaunch-phase-b):
 *   Launch a NEW E2E app with the same DB → assert recovery banner + Review
 *   Preview still work → write evidence.
 *
 * Fallback: if the E2E binary is missing, run the SQLite reopen Rust test and
 * record mode=sqlite_reopen_proxy (not process relaunch).
 *
 *   node e2e/process-relaunch-recovery.mjs
 */

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const evidencePath = path.resolve(
  repoRoot,
  "reports/process-relaunch-recovery-results.json",
);
const statePath = path.resolve(repoRoot, "reports/process-relaunch-state.json");
const wdioConfig = path.join(__dirname, "wdio.conf.ts");

function resolveE2eBinary() {
  const fromEnv = process.env.CORESIDE_E2E_BINARY?.trim();
  if (fromEnv && fs.existsSync(fromEnv)) return fromEnv;
  const candidates = [
    path.join(repoRoot, "src-tauri/target/debug/Coreside"),
    path.join(repoRoot, "src-tauri/target/debug/coreside"),
  ];
  return candidates.find((p) => fs.existsSync(p)) ?? null;
}

function runSqliteReopenProxy(startedAt, reason) {
  const rust = spawnSync(
    "cargo",
    [
      "test",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--lib",
      "interrupted_checkpoint_survives_sqlite_reopen",
      "--",
      "--nocapture",
    ],
    {
      cwd: repoRoot,
      encoding: "utf8",
      env: { ...process.env, CARGO_TARGET_DIR: undefined },
    },
  );
  const passed =
    rust.status === 0 && /test result: ok/.test(`${rust.stdout}\n${rust.stderr}`);
  const evidence = {
    harness: "process-relaunch-recovery",
    mode: "sqlite_reopen_proxy",
    evidenceLevel: "SQLite Reopen Verified",
    note: reason,
    startedAt,
    finishedAt: new Date().toISOString(),
    host: os.hostname(),
    status: passed ? "passed" : "failed",
    rustExitCode: rust.status,
    rustStdoutTail: (rust.stdout || "").split("\n").slice(-20),
    rustStderrTail: (rust.stderr || "").split("\n").slice(-20),
  };
  fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
  fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));
  console.log(JSON.stringify(evidence, null, 2));
  process.exit(passed ? 0 : 1);
}

function runWdioSuite(suite, env) {
  console.log(`[process-relaunch] suite=${suite}`);
  const result = spawnSync(
    "npx",
    ["wdio", "run", wdioConfig, "--suite", suite],
    {
      cwd: repoRoot,
      env,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  const out = `${result.stdout || ""}\n${result.stderr || ""}`;
  if (result.status !== 0) {
    console.error(out.slice(-8000));
  }
  return { status: result.status ?? 1, out };
}

const startedAt = new Date().toISOString();
const binary = resolveE2eBinary();
if (!binary) {
  runSqliteReopenProxy(
    startedAt,
    "E2E binary missing — ran SQLite reopen proxy only. Run npm run e2e:build first for Full OS Process Relaunch Verified.",
  );
}

const dbDir = fs.mkdtempSync(path.join(os.tmpdir(), "coreside-relaunch-"));
const dbPath = path.join(dbDir, "coreside.db");
try {
  fs.unlinkSync(statePath);
} catch {
  /* absent is fine */
}

const commit = spawnSync("git", ["rev-parse", "HEAD"], {
  cwd: repoRoot,
  encoding: "utf8",
});
const porcelain = spawnSync("git", ["status", "--porcelain"], {
  cwd: repoRoot,
  encoding: "utf8",
});

const baseEnv = {
  ...process.env,
  CORESIDE_E2E: "1",
  CORESIDE_DB_PATH: dbPath,
  CORESIDE_E2E_BINARY: binary,
  AI_PROVIDER: process.env.AI_PROVIDER || "mock",
  CORESIDE_E2E_SEED: "existing",
  CORESIDE_E2E_COMMIT: (commit.stdout || "").trim(),
  CORESIDE_E2E_DIRTY: (porcelain.stdout || "").trim().length > 0 ? "1" : "0",
  CORESIDE_PROCESS_RELAUNCH_STATE: statePath,
  NO_PROXY: "127.0.0.1,localhost",
  no_proxy: "127.0.0.1,localhost",
};
delete baseEnv.CARGO_TARGET_DIR;

const phaseA = runWdioSuite("process-relaunch-phase-a", baseEnv);
if (phaseA.status !== 0) {
  const evidence = {
    harness: "process-relaunch-recovery",
    mode: "os_process_relaunch",
    evidenceLevel: "Native Debug Desktop E2E Verified",
    phase: "A",
    status: "failed",
    startedAt,
    finishedAt: new Date().toISOString(),
    dbPath,
    binary,
    exitCode: phaseA.status,
  };
  fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
  fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));
  console.error("[process-relaunch] Phase A failed");
  process.exit(phaseA.status);
}

if (!fs.existsSync(statePath)) {
  console.error("[process-relaunch] Phase A did not write state file");
  process.exit(1);
}

// Process A has exited — Phase B is a new OS process lifetime on the same DB.
const phaseB = runWdioSuite("process-relaunch-phase-b", baseEnv);
const state = JSON.parse(fs.readFileSync(statePath, "utf8"));
const passed = phaseB.status === 0;
const evidence = {
  harness: "process-relaunch-recovery",
  mode: "os_process_relaunch",
  evidenceLevel: passed
    ? "Full OS Process Relaunch Verified"
    : "Native Debug Desktop E2E Verified",
  note: "Two sequential WDIO sessions; embedded WebDriver lives inside each Coreside E2E process. Supervisor process survives both.",
  startedAt,
  finishedAt: new Date().toISOString(),
  status: passed ? "passed" : "failed",
  dbPath: "<isolated-temp-db>",
  binary: "Coreside",
  phaseAExit: phaseA.status,
  phaseBExit: phaseB.status,
  state,
};
fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2));
console.log(JSON.stringify(evidence, null, 2));

if (passed) {
  try {
    fs.rmSync(dbDir, { recursive: true, force: true });
  } catch {
    /* best-effort */
  }
} else {
  console.error(`[process-relaunch] left DB at ${dbDir} for inspection`);
}
process.exit(passed ? 0 : 1);
