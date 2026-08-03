#!/usr/bin/env node
/**
 * Coreside assurance validator — validates existing evidence reports.
 *
 * This is NOT a quick development check and must never claim public-beta
 * readiness from incomplete/stale/partial evidence.
 *
 * Usage: npm run assurance:report
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const reportsDir = path.join(root, "reports");
const outPath = path.join(reportsDir, "assurance-report.json");

function git(args) {
  const r = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  return (r.stdout || "").trim();
}

function readJson(file) {
  const p = path.join(reportsDir, file);
  if (!fs.existsSync(p)) return { missing: true, path: p };
  try {
    return { missing: false, path: p, data: JSON.parse(fs.readFileSync(p, "utf8")) };
  } catch (e) {
    return { missing: false, path: p, parseError: String(e.message || e) };
  }
}

const commit = git(["rev-parse", "HEAD"]);
const dirty = git(["status", "--porcelain"]).length > 0;
const required = [
  "current-source-baseline.json",
  "current-source-fingerprint.json",
  "release-gates.json",
];

const findings = [];
let failed = false;

for (const file of required) {
  const r = readJson(file);
  if (r.missing) {
    findings.push({ severity: "P1", file, status: "missing" });
    failed = true;
    continue;
  }
  if (r.parseError) {
    findings.push({ severity: "P1", file, status: "invalid_json", detail: r.parseError });
    failed = true;
    continue;
  }
  const d = r.data;
  if (d.commit && d.commit !== commit) {
    findings.push({
      severity: "P1",
      file,
      status: "stale_commit",
      reportCommit: d.commit,
      activeCommit: commit,
    });
    failed = true;
  }
  if (d.exitStatus === "passed_partial" || d.status === "partial" || d.result === "partial") {
    findings.push({ severity: "P1", file, status: "partial_rejected" });
    failed = true;
  }
}

const baseline = readJson("current-source-baseline.json");
if (!baseline.missing && baseline.data?.publicBeta === "Public beta ready" && dirty) {
  findings.push({
    severity: "P1",
    file: "current-source-baseline.json",
    status: "dirty_tree_cannot_claim_beta",
  });
  failed = true;
}

const quickEvidence = readJson("release-evidence.json");
if (
  !quickEvidence.missing &&
  quickEvidence.data?.command?.includes("--quick") &&
  (quickEvidence.data?.publicBetaReady === true ||
    quickEvidence.data?.verdict === "Public beta ready")
) {
  findings.push({
    severity: "P1",
    file: "release-evidence.json",
    status: "quick_evidence_cannot_claim_beta",
  });
  failed = true;
}

const report = {
  schemaVersion: 1,
  product: "Coreside",
  generatedAt: new Date().toISOString(),
  commit,
  dirty,
  command: "assurance:report",
  exitStatus: failed ? "failed" : "passed",
  note: "Validates report presence/freshness/partial rejection. Does not execute the full release suite.",
  findings,
  localFirstVerdict: "Not ready",
  hostedAiVerdict: "Not ready",
};

fs.mkdirSync(reportsDir, { recursive: true });
fs.writeFileSync(outPath, JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify({ exitStatus: report.exitStatus, findings: findings.length, outPath }, null, 2));
process.exit(failed ? 1 : 0);
