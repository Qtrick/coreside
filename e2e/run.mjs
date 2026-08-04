#!/usr/bin/env node
/**
 * Desktop E2E orchestrator.
 *
 * Runs WDIO suites with isolated CORESIDE_DB_PATH profiles:
 * 1. main — clean DB (Journeys 1, 3, 4)
 * 2. existing-* — seeded profile, one fresh DB per journey group (2, 5–11, 14)
 * 3. true-streaming / wallpaper-targeted — clean DB (Journeys 12–13)
 * 4. Journeys 15–16 (onboarding) are registered as not_run — CORESIDE_E2E
 *    disables onboarding; specs exist under e2e/specs/ but are not executed here.
 *
 * Writes reports/e2e-results.json from actual suite exits — never invents passes.
 * Journey 7 is recorded as passed_partial when its suite exits 0 (coverage is
 * intentionally incomplete; see docs/E2E_EXECUTION.md). Overall status becomes
 * passed_partial whenever any journey is partial — never inflate to a full pass.
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
  {
    id: 12,
    name: "true-streaming",
    suite: "true-streaming",
    coverage: "full",
  },
  {
    id: 13,
    name: "wallpaper-targeted",
    suite: "wallpaper-targeted",
    coverage: "full",
  },
  {
    id: 14,
    name: "stream-eavesdropping-denial",
    suite: "existing-eavesdrop",
    coverage: "full",
  },
  {
    id: 15,
    name: "first-run-welcome",
    suite: "first-run-welcome",
    coverage: "full",
    note: "Spec present; not executed — CORESIDE_E2E disables onboarding",
  },
  {
    id: 16,
    name: "core-tutorial",
    suite: "core-tutorial",
    coverage: "full",
    note: "Spec present; not executed — CORESIDE_E2E disables onboarding",
  },
];

/** @type {Map<string, "passed" | "failed" | "not_run">} */
const suiteOutcomes = new Map(JOURNEYS.map((j) => [j.suite, "not_run"]));

function writeResults(overallStatus) {
  const commitEnv = process.env.CORESIDE_E2E_COMMIT?.trim();
  const dirtyEnv = process.env.CORESIDE_E2E_DIRTY;
  const commit =
    commitEnv ||
    (
      spawnSync("git", ["rev-parse", "HEAD"], {
        cwd: root,
        encoding: "utf8",
      }).stdout || ""
    ).trim() ||
    null;
  let dirty = null;
  if (dirtyEnv === "0" || dirtyEnv === "1") {
    dirty = dirtyEnv === "1";
  } else {
    dirty =
      (
        spawnSync("git", ["status", "--porcelain"], {
          cwd: root,
          encoding: "utf8",
        }).stdout || ""
      ).trim().length > 0;
  }
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
  const hasFailed = journeys.some((j) => j.status === "failed");
  const hasPartial = journeys.some((j) => j.status === "passed_partial");
  let status = overallStatus;
  if (hasFailed) {
    status = "failed";
  } else if (hasPartial && (overallStatus === "passed" || overallStatus === "passed_partial")) {
    // Journey 7 (and any future partial) must not inflate a full desktop pass.
    status = "passed_partial";
  }
  const payload = {
    product: "Coreside",
    generatedAt: new Date().toISOString(),
    commit,
    dirty,
    command: "npm run e2e",
    status,
    platform: os.platform(),
    arch: os.arch(),
    evidenceLevel:
      status === "passed" ? "Desktop Verified" : "not_desktop_verified",
    journeys,
  };
  fs.mkdirSync(path.dirname(resultsPath), { recursive: true });
  fs.writeFileSync(resultsPath, JSON.stringify(payload, null, 2) + "\n");
  console.log(`[e2e:run] wrote ${path.relative(root, resultsPath)} status=${overallStatus}`);
}

function runSuite(suite, { seed } = {}) {
  const dbDir = fs.mkdtempSync(path.join(os.tmpdir(), `coreside-e2e-${suite}-`));
  const dbPath = path.join(dbDir, "coreside.db");
  const commit = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: root,
    encoding: "utf8",
  });
  const porcelain = spawnSync("git", ["status", "--porcelain"], {
    cwd: root,
    encoding: "utf8",
  });
  const env = {
    ...process.env,
    CORESIDE_E2E: "1",
    CORESIDE_DB_PATH: dbPath,
    AI_PROVIDER: process.env.AI_PROVIDER || "mock",
    CORESIDE_E2E_COMMIT: (commit.stdout || "").trim(),
    CORESIDE_E2E_DIRTY: (porcelain.stdout || "").trim().length > 0 ? "1" : "0",
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

// Clean-profile suites (mock AI is enough for live stream probe / wallpaper).
runSuite("true-streaming");
runSuite("wallpaper-targeted");

// Seeded eavesdropping denial (needs tool window from existing seed).
runSuite("existing-eavesdrop", { seed: "existing" });

writeResults("passed");
console.log("\n[e2e:run] all suites passed");
