#!/usr/bin/env node
/**
 * Supervised process-relaunch harness for generation recovery durability.
 *
 * Phase A: open an isolated SQLite DB, write an interrupted_recoverable
 * checkpoint via the Rust test binary path is preferred; this script documents
 * the external supervisor contract for desktop relaunch:
 *
 *   1. Launch E2E Coreside with CORESIDE_DB_PATH=<isolated>
 *   2. Create recoverable staging (Journey 34 cancel path)
 *   3. Terminate ONLY that E2E process (WebDriver port + binary match)
 *   4. Relaunch with the same CORESIDE_DB_PATH
 *   5. Assert recovery banner still lists the turn
 *
 * Full OS relaunch inside one embedded WebDriver session kills the driver port
 * (see Journey 29 note). This supervisor is intended to run *outside* WDIO as:
 *
 *   node e2e/process-relaunch-recovery.mjs
 *
 * Until automated Phase A/B desktop orchestration is wired, SQLite reopen
 * durability is proven by:
 *   cargo test --lib interrupted_checkpoint_survives_sqlite_reopen
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
  note:
    "Full OS process relaunch under embedded WebDriver remains environmentally blocked; this harness proves checkpoint durability across Database::open_path reopen (authoritative SQLite identity).",
  startedAt: new Date().toISOString(),
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
