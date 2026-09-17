#!/usr/bin/env node
/**
 * Coreside release evidence — runs offline gates and writes machine-readable JSON.
 *
 * Never invents pass/fail. If a command was not run, status is "not_run".
 * If parsing a count fails, stores raw summary instead of a fabricated count.
 *
 * Usage:
 *   npm run release:evidence
 *   npm run release:evidence -- --quick   # doctor + inventories only (no full suite)
 */

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const quick = process.argv.includes("--quick");
const reportsDir = path.join(root, "reports");
const outPath = path.join(reportsDir, "release-evidence.json");

function git(args) {
  const r = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  return (r.stdout || "").trim();
}

function runGate(id, command, args, opts = {}) {
  const started = Date.now();
  const r = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env, ...opts.env },
    maxBuffer: 16 * 1024 * 1024,
    shell: false,
  });
  const durationMs = Date.now() - started;
  const stdoutFull = r.stdout || "";
  const stderrFull = r.stderr || "";
  const exitCode = r.status ?? (r.error ? 1 : 0);
  const status = r.error
    ? "blocked"
    : exitCode === 0
      ? "passed"
      : "failed";
  return {
    id,
    command: [command, ...args].join(" "),
    exitCode,
    status,
    durationMs,
    // Parse counts from the full streams; only bound what we persist as rawTail.
    summary: summarize(id, stdoutFull, stderrFull, exitCode),
    rawTail: (stdoutFull + "\n" + stderrFull).trim().slice(-2000),
    error: r.error ? String(r.error.message || r.error) : undefined,
  };
}

function summarize(id, stdout, stderr, exitCode) {
  const text = `${stdout}\n${stderr}`;
  if (id === "vitest") {
    const m = text.match(/Tests\s+(\d+)\s+passed/);
    return m
      ? { testsPassed: Number(m[1]) }
      : { note: "count_unparsed", rawHint: "look for 'Tests N passed'" };
  }
  if (id === "rust-tests" || id === "migration-fixtures" || id === "registered-actions") {
    const matches = [...text.matchAll(/test result:\s*ok\.\s+(\d+)\s+passed/gi)];
    if (matches.length) {
      const counts = matches.map((m) => Number(m[1]));
      return { testsPassed: Math.max(...counts), resultLines: counts.length };
    }
    return { note: "count_unparsed" };
  }
  if (id === "doctor") {
    const all = text.match(/All\s+(\d+)\s+checks passed/i);
    if (all) {
      const n = Number(all[1]);
      return { checksPassed: n, checksTotal: n };
    }
    const m = text.match(/(\d+)\s*\/\s*(\d+)/);
    return m
      ? { checksPassed: Number(m[1]), checksTotal: Number(m[2]) }
      : { note: "count_unparsed" };
  }
  if (id === "lint") {
    const warnings = (text.match(/\bwarning\b/gi) || []).length;
    return { exitCode, warningMentions: warnings };
  }
  if (id === "build-web") {
    const chunks = [
      ...text.matchAll(/(dist\/assets\/[\w.-]+\.js)\s+([\d.]+)\s+kB/g),
    ].map((m) => ({ file: m[1], kb: Number(m[2]) }));
    if (!chunks.length) return { note: "size_unparsed" };
    // Prefer the largest JS chunk as the production main bundle (entry can be small).
    const main = [...chunks].sort((a, b) => b.kb - a.kb)[0];
    const mocks = chunks.find((c) => /mocks/i.test(c.file));
    return {
      jsChunkKb: chunks.map((c) => c.kb),
      mainJsKb: main.kb,
      mainJsFile: main.file,
      mocksJsKb: mocks?.kb ?? null,
    };
  }
  return { exitCode };
}

function readJsonSafe(rel) {
  const p = path.join(root, rel);
  if (!fs.existsSync(p)) return null;
  try {
    return JSON.parse(fs.readFileSync(p, "utf8"));
  } catch {
    return { error: "parse_failed", path: rel };
  }
}

function countRegisteredCommands() {
  const inv = readJsonSafe("reports/tauri-command-inventory.json");
  if (!inv) return { status: "missing_inventory" };
  const cmds = inv.commands || inv.registered || inv.items;
  if (Array.isArray(cmds)) return { count: cmds.length, source: "inventory" };
  if (typeof inv.commandCount === "number") {
    return { count: inv.commandCount, source: "inventory" };
  }
  // Fallback: count .invoke( / command! registrations in lib.rs
  const lib = fs.readFileSync(path.join(root, "src-tauri/src/lib.rs"), "utf8");
  const matches = lib.match(/commands::\w+/g) || [];
  return { count: new Set(matches).size, source: "lib.rs_heuristic" };
}

function toolchain() {
  const node = spawnSync("node", ["-v"], { encoding: "utf8" });
  const rustc = spawnSync("rustc", ["--version"], { encoding: "utf8" });
  const cargo = spawnSync("cargo", ["--version"], { encoding: "utf8" });
  return {
    node: (node.stdout || "").trim() || null,
    rustc: (rustc.stdout || "").trim() || null,
    cargo: (cargo.stdout || "").trim() || null,
    os: `${os.platform()} ${os.release()} (${os.arch()})`,
  };
}

fs.mkdirSync(reportsDir, { recursive: true });

const commit = git(["rev-parse", "HEAD"]);
const dirty = git(["status", "--porcelain"]);
const branch = git(["rev-parse", "--abbrev-ref", "HEAD"]);

/** @type {Record<string, unknown>[]} */
const gates = [];

function add(gate) {
  gates.push(gate);
  const mark =
    gate.status === "passed"
      ? "PASS"
      : gate.status === "failed"
        ? "FAIL"
        : gate.status === "passed_partial"
          ? "PARTIAL"
          : gate.status.toUpperCase();
  console.log(`[evidence] ${mark} ${gate.id} (${gate.durationMs ?? "—"}ms)`);
}

// Always-run light gates
add(
  runGate("doctor", "npm", ["run", "doctor"], {
    env: { RUNNING_RELEASE_EVIDENCE: "1" },
  }),
);
add(
  runGate("package-json-parse", "node", [
    "--input-type=module",
    "-e",
    "import fs from 'node:fs'; JSON.parse(fs.readFileSync('package.json','utf8')); console.log('ok');",
  ]),
);

if (!quick) {
  add(runGate("typecheck", "npm", ["run", "typecheck"]));
  add(runGate("lint", "npm", ["run", "lint"]));
  add(runGate("vitest", "npm", ["test"]));
  add(runGate("rust-check", "npm", ["run", "check:rust"]));
  add(runGate("rust-tests", "npm", ["run", "test:rust"]));
  add(runGate("migration-fixtures", "npm", ["run", "test:migrations"]));
  add(runGate("registered-actions", "npm", ["run", "test:registered-actions"]));
  add(runGate("fmt-check", "npm", ["run", "fmt:check"]));
  add(runGate("build-web", "npm", ["run", "build:web"]));
  // Packaged / E2E — prefer previously generated reports when present; never invent pass.
  const packagingReport = readJsonSafe("reports/packaging-results.json");
  if (packagingReport?.status === "passed" || packagingReport?.status === "passed_with_warnings") {
    gates.push({
      id: "packaged-tauri-build",
      command: "npm run build && npm run package:scan",
      status: "passed",
      exitCode: 0,
      summary: {
        fromReport: "reports/packaging-results.json",
        packagingStatus: packagingReport.status,
        localMacosBuild: packagingReport.localMacosBuild ?? null,
        note: "Consumed packaging-results.json (scan + prior local build evidence)",
      },
    });
  } else {
    gates.push({
      id: "packaged-tauri-build",
      command: "npm run build",
      status: "manual_verification_required",
      exitCode: null,
      summary: {
        note: "Run npm run build separately; record artifact paths in packaging-results.json",
      },
    });
  }

  const e2eReport = readJsonSafe("reports/e2e-results.json");
  if (e2eReport?.status === "passed") {
    gates.push({
      id: "desktop-e2e",
      command: e2eReport.command || "npm run e2e",
      status: "passed",
      exitCode: 0,
      summary: {
        fromReport: "reports/e2e-results.json",
        journeys: e2eReport.journeys ?? null,
        note: "Consumed e2e-results.json written by the E2E runner",
      },
    });
  } else if (e2eReport?.status === "passed_partial") {
    // Honest: partial journeys (7/10) must not inflate a full "passed" gate.
    gates.push({
      id: "desktop-e2e",
      command: e2eReport.command || "npm run e2e",
      status: "passed_partial",
      exitCode: 0,
      summary: {
        fromReport: "reports/e2e-results.json",
        journeys: e2eReport.journeys ?? null,
        note: "E2E finished with intentional passed_partial coverage — not a full pass",
      },
    });
  } else {
    gates.push({
      id: "desktop-e2e",
      command: "npm run e2e:desktop",
      status: "manual_verification_required",
      exitCode: null,
      summary: {
        note: "Requires GUI session; run npm run e2e:desktop and merge e2e-results.json",
      },
    });
  }
}

// Never mark a gate "passed" unless exitCode === 0 (defense in depth).
for (const g of gates) {
  if (
    g.status === "passed" &&
    (typeof g.exitCode !== "number" || g.exitCode !== 0)
  ) {
    g.status = "failed";
    g.summary = {
      ...(typeof g.summary === "object" && g.summary ? g.summary : {}),
      note: "corrected_invented_pass",
    };
  }
}

const failed = gates.filter((g) => g.status === "failed" || g.status === "blocked");

const localByokVerdict = (() => {
  if (failed.length > 0) return "gates_failed";
  // Quick mode only runs doctor + inventories — do not claim full offline green.
  if (quick) return "quick_partial_full_suite_not_run";
  // Honest: passed_partial (e.g. E2E 7/10) is not a full green offline suite.
  if (gates.some((g) => g.status === "passed_partial")) {
    return "gates_partial_pending_packaging_e2e";
  }
  return "gates_green_pending_packaging_e2e";
})();

const evidence = {
  product: "Coreside",
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  git: {
    commit,
    branch,
    dirty: dirty.length > 0,
    dirtyFileCount: dirty ? dirty.split("\n").filter(Boolean).length : 0,
  },
  toolchain: toolchain(),
  mode: quick ? "quick" : "offline-gates",
  tracks: {
    localByok: {
      name: "Local-first BYOK Coreside",
      verdict: localByokVerdict,
      note: "Hosted AI blockers are tracked separately and must not fail this verdict.",
    },
    hostedAi: {
      name: "Hosted Coreside AI",
      verdict: "separate_track",
      note: "See reports/hosted-ai-readiness.json — not mixed into local beta.",
    },
  },
  gates,
  tallies: {
    passed: gates.filter((g) => g.status === "passed").length,
    passedPartial: gates.filter((g) => g.status === "passed_partial").length,
    failed: gates.filter((g) => g.status === "failed" || g.status === "blocked")
      .length,
    manualVerificationRequired: gates.filter(
      (g) => g.status === "manual_verification_required",
    ).length,
    notRun: gates.filter((g) => g.status === "not_run").length,
  },
  inventories: {
    commands: countRegisteredCommands(),
    migrations: fs
      .readdirSync(path.join(root, "src-tauri/migrations"))
      .filter((f) => f.endsWith(".sql")).length,
  },
  supersededReports: [
    {
      path: "reports/release-gates.json",
      assessedAt: "2026-07-19",
      status: "historical",
      note: "Superseded by reports/release-evidence.json",
    },
  ],
};

fs.writeFileSync(outPath, JSON.stringify(evidence, null, 2) + "\n");

// Lightweight companion readiness files (consume evidence; do not duplicate counts by hand)
const localBeta = {
  product: "Coreside",
  track: "local-byok",
  generatedAt: evidence.generatedAt,
  commit,
  dirty: evidence.git.dirty,
  sourceEvidence: "reports/release-evidence.json",
  offlineGates: evidence.tallies,
  verdict: evidence.tracks.localByok.verdict,
  mode: evidence.mode,
};
fs.writeFileSync(
  path.join(reportsDir, "local-beta-readiness.json"),
  JSON.stringify(localBeta, null, 2) + "\n",
);

const hosted = readJsonSafe("reports/hosted-ai-readiness.json") || {
  product: "Coreside",
  track: "hosted-ai",
  verdict: "not_ready",
  note: "Hosted AI private alpha is a separate track from local BYOK beta.",
};
hosted.generatedAt = evidence.generatedAt;
hosted.commit = commit;
hosted.sourceEvidence = "reports/release-evidence.json";
hosted.note =
  hosted.note ||
  "Do not mix hosted P0 blockers into local-first beta readiness.";
fs.writeFileSync(
  path.join(reportsDir, "hosted-ai-readiness.json"),
  JSON.stringify(hosted, null, 2) + "\n",
);

const finalFailed = evidence.tallies.failed;
console.log(`\n[evidence] wrote ${path.relative(root, outPath)}`);
console.log(
  `[evidence] passed=${evidence.tallies.passed} failed=${finalFailed} manual=${localBeta.offlineGates.manualVerificationRequired} verdict=${localByokVerdict}`,
);
process.exit(finalFailed > 0 ? 1 : 0);
