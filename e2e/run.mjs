#!/usr/bin/env node
/**
 * Desktop E2E orchestrator.
 *
 * Runs WDIO suites with isolated CORESIDE_DB_PATH profiles:
 * 1. main — clean DB (Journeys 1, 3, 4)
 * 2. existing-* — seeded profile, one fresh DB per journey group (2, 5–10)
 *
 * Writes reports/e2e-results.json from actual suite exits — never invents passes.
 * Journeys 7 and 10 are recorded as passed_partial when their suite exits 0
 * (coverage is intentionally incomplete; see docs/E2E_EXECUTION.md).
 */

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const wdioConfig = path.join(__dirname, "wdio.conf.ts");
const resultsPath = path.join(root, "reports/e2e-results.json");

/** @type {{ id: number, name: string, suite: string, coverage: "full" | "partial", note?: string }[]} */
const JOURNEYS = [
  { id: 1, name: "clean-startup", suite: "main", coverage: "full" },
  { id: 2, name: "existing-profile", suite: "existing-chat", coverage: "full" },
  { id: 3, name: "settings", suite: "main", coverage: "full" },
  { id: 4, name: "conversation-draft", suite: "main", coverage: "full" },
  { id: 5, name: "generated-tool-state", suite: "existing-tool", coverage: "full" },
  { id: 6, name: "approval-approve-once", suite: "existing-approval", coverage: "full" },
  {
    id: 7,
    name: "multi-window-approval-race",
    suite: "existing-approval-race",
    coverage: "partial",
    note: "Secondary window opens; duplicate-approval UI on secondary not fully asserted",
  },
  { id: 8, name: "grant-revoke", suite: "existing-grant", coverage: "full" },
  { id: 9, name: "recovery-mode", suite: "existing-recovery", coverage: "full" },
  {
    id: 10,
    name: "secondary-window",
    suite: "existing-window",
    coverage: "full",
  },
  {
    id: 11,
    name: "command-authority-denial",
    suite: "existing-authority",
    coverage: "full",
  },
];

/** @type {Map<string, "passed" | "failed" | "not_run">} */
const suiteOutcomes = new Map(JOURNEYS.map((j) => [j.suite, "not_run"]));

function writeResults(overallStatus) {
  const commit = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: root,
    encoding: "utf8",
  });
  const journeys = JOURNEYS.map((j) => {
    const suiteStatus = suiteOutcomes.get(j.suite) ?? "not_run";
    let status = suiteStatus;
    if (suiteStatus === "passed" && j.coverage === "partial") {
      status = "passed_partial";
    }
    return {
      id: j.id,
      name: j.name,
      status,
      ...(j.note && status === "passed_partial" ? { note: j.note } : {}),
    };
  });
  const payload = {
    product: "Coreside",
    generatedAt: new Date().toISOString(),
    commit: (commit.stdout || "").trim() || null,
    command: "npm run e2e",
    status: overallStatus,
    platform: `${os.platform()}`,
    journeys,
  };
  fs.mkdirSync(path.dirname(resultsPath), { recursive: true });
  fs.writeFileSync(resultsPath, JSON.stringify(payload, null, 2) + "\n");
  console.log(`[e2e:run] wrote ${path.relative(root, resultsPath)} status=${overallStatus}`);
}

function runSuite(suite, { seed } = {}) {
  const dbDir = fs.mkdtempSync(path.join(os.tmpdir(), `coreside-e2e-${suite}-`));
  const dbPath = path.join(dbDir, "coreside.db");
  const env = {
    ...process.env,
    CORESIDE_E2E: "1",
    CORESIDE_DB_PATH: dbPath,
    AI_PROVIDER: process.env.AI_PROVIDER || "mock",
  };
  // Explicitly unset vs empty-string: a polluted parent shell must not leak seed.
  if (seed) {
    env.CORESIDE_E2E_SEED = seed;
  } else {
    delete env.CORESIDE_E2E_SEED;
  }
  // Drop any inherited CARGO_TARGET_DIR so binary resolution stays local.
  delete env.CARGO_TARGET_DIR;
  console.log(`\n[e2e:run] suite=${suite} db=${dbPath} seed=${seed ?? "(none)"}`);
  const result = spawnSync(
    "npx",
    ["wdio", "run", wdioConfig, "--suite", suite],
    { cwd: root, env, stdio: "inherit" },
  );
  if (result.status !== 0) {
    suiteOutcomes.set(suite, "failed");
    writeResults("failed");
    console.error(
      `[e2e:run] suite=${suite} failed; leaving isolated DB at ${dbDir} for inspection`,
    );
    process.exit(result.status ?? 1);
  }
  suiteOutcomes.set(suite, "passed");
  // Success: drop temp profile so repeated local runs do not fill $TMPDIR.
  try {
    fs.rmSync(dbDir, { recursive: true, force: true });
  } catch (err) {
    console.warn(`[e2e:run] cleanup warning for ${dbDir}: ${err}`);
  }
}

runSuite("main");

const existingSuites = [
  "existing-chat",
  "existing-tool",
  "existing-approval-race",
  "existing-approval",
  "existing-grant",
  "existing-recovery",
  "existing-window",
  "existing-authority",
];

for (const suite of existingSuites) {
  runSuite(suite, { seed: "existing" });
}

writeResults("passed");
console.log("\n[e2e:run] all suites passed");
