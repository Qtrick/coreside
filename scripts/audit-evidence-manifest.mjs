#!/usr/bin/env node
/**
 * Build and validate reports/evidence-manifest.json.
 *
 * Fails when a required current report is missing, unreadable, claims a higher
 * evidence level than its artifacts prove, or disagrees on source identity.
 *
 * Usage: npm run audit:evidence-manifest
 */
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const reportsDir = path.join(root, "reports");
const outPath = path.join(reportsDir, "evidence-manifest.json");
const commandName = "audit:evidence-manifest";

function git(args) {
  const r = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  return (r.stdout || "").trim();
}

function sha256File(filePath) {
  return crypto.createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
}

function readJson(abs) {
  try {
    return { ok: true, data: JSON.parse(fs.readFileSync(abs, "utf8")) };
  } catch (e) {
    return { ok: false, error: String(e.message || e) };
  }
}

/** Required current gates. Historical-only rows may be listed but do not satisfy gates. */
const REQUIRED = [
  {
    path: "reports/current-source-baseline.json",
    evidenceType: "source-baseline",
    requiredGate: "source-identity",
  },
  {
    path: "reports/current-source-fingerprint.json",
    evidenceType: "source-fingerprint",
    requiredGate: "source-identity",
  },
  {
    path: "reports/active-versus-uploaded-coreside.json",
    evidenceType: "archive-delta",
    requiredGate: "source-identity",
  },
  {
    path: "reports/report-freshness-inventory.json",
    evidenceType: "freshness",
    requiredGate: "source-identity",
  },
  {
    path: "reports/readiness-ladder.json",
    evidenceType: "readiness",
    requiredGate: "readiness",
  },
  {
    path: "reports/readiness-delta.json",
    evidenceType: "readiness-delta",
    requiredGate: "readiness",
  },
  {
    path: "reports/e2e-results.json",
    evidenceType: "desktop-e2e",
    requiredGate: "desktop-e2e",
  },
  {
    path: "reports/packaged-smoke-results.json",
    evidenceType: "packaged-launch-smoke",
    requiredGate: "packaged-launch-smoke",
  },
  {
    path: "reports/assurance-report.json",
    evidenceType: "assurance",
    requiredGate: "assurance",
  },
  {
    path: "reports/release-evidence.json",
    evidenceType: "release-evidence",
    requiredGate: "release",
  },
  {
    path: "reports/local-beta-readiness.json",
    evidenceType: "local-beta",
    requiredGate: "readiness",
  },
];

const OVERCLAIM_RULES = [
  {
    path: "reports/packaged-smoke-results.json",
    forbidEvidenceLevels: ["Packaged Verified", "Cross-Platform Verified", "Human Accepted"],
    when: (d) =>
      d?.classification === "Packaged launch smoke" ||
      d?.smokeKind === "launch_quit" ||
      d?.scope === "launch_quit" ||
      (Array.isArray(d?.checks) &&
        d.checks.every((c) =>
          ["artifact_scan", "launch", "quit", "process_termination"].includes(c?.id || c?.name),
        )),
    reason: "Launch/quit packaged smoke must not claim Packaged Verified",
  },
  {
    path: "reports/e2e-results.json",
    forbidEvidenceLevels: ["Desktop Verified", "Packaged Verified"],
    when: (d) =>
      d?.status === "passed_partial" ||
      d?.evidenceLevel === "not_desktop_verified" ||
      (Array.isArray(d?.journeys) &&
        d.journeys.some((j) => j?.status === "passed_partial" || j?.status === "partial")),
    reason: "Partial E2E suite must not claim Desktop Verified",
  },
  {
    path: "reports/command-authority-results.json",
    forbidEvidenceLevels: ["Desktop Verified"],
    when: (d) =>
      d?.rawInvokeDenialTest === "passed" && d?.status === "generated_from_e2e",
    reason:
      "Journey 11 isolated evidence must use Isolated Desktop Verified, not bare Desktop Verified",
  },
  {
    path: "reports/command-authority-results.json",
    forbidEvidenceLevels: ["Desktop Verified", "Packaged Verified"],
    when: (d) =>
      d?.rawInvokeDenialTest === "passed" &&
      d?.status === "generated_from_e2e" &&
      d?.isolatedSuiteOnly !== true,
    reason: "Journey 11 ACL evidence must declare isolatedSuiteOnly: true",
  },
];

const commit = git(["rev-parse", "HEAD"]);
const dirty = git(["status", "--porcelain"]).length > 0;
const fingerprintPath = path.join(reportsDir, "current-source-fingerprint.json");
let sourceFingerprint = null;
let packageLockHash = null;
let cargoLockHash = null;
if (fs.existsSync(fingerprintPath)) {
  const fp = readJson(fingerprintPath);
  if (fp.ok) {
    sourceFingerprint = fp.data.sourceFingerprint || fp.data.fingerprint || null;
    packageLockHash = fp.data.packageLockHash || null;
    cargoLockHash = fp.data.cargoLockHash || null;
  }
}

const findings = [];
const entries = [];
let failed = false;

for (const req of REQUIRED) {
  const abs = path.join(root, req.path);
  const present = fs.existsSync(abs);
  const entry = {
    path: req.path,
    sha256: present ? sha256File(abs) : null,
    evidenceType: req.evidenceType,
    sourceFingerprint: null,
    commit: null,
    dirty: null,
    platform: null,
    architecture: null,
    generatedAt: null,
    command: null,
    requiredGate: req.requiredGate,
    currentOrHistorical: "current",
    presentOrMissing: present ? "present" : "missing",
    evidenceLevel: null,
    exitStatus: null,
  };

  if (!present) {
    findings.push({
      severity: "P0",
      path: req.path,
      status: "missing",
      detail: "Required evidence file is absent",
    });
    failed = true;
    entries.push(entry);
    continue;
  }

  const parsed = readJson(abs);
  if (!parsed.ok) {
    findings.push({
      severity: "P0",
      path: req.path,
      status: "invalid_json",
      detail: parsed.error,
    });
    failed = true;
    entries.push(entry);
    continue;
  }

  const d = parsed.data;
  entry.sourceFingerprint = d.sourceFingerprint || d.fingerprint || null;
  entry.commit = d.commit || null;
  entry.dirty = typeof d.dirty === "boolean" ? d.dirty : null;
  entry.platform = d.platform || null;
  entry.architecture = d.architecture || null;
  entry.generatedAt = d.generatedAt || null;
  entry.command = d.command || null;
  entry.evidenceLevel = d.evidenceLevel || d.classification || null;
  entry.exitStatus = d.exitStatus || d.status || null;

  if (entry.commit && entry.commit !== commit) {
    findings.push({
      severity: "P1",
      path: req.path,
      status: "commit_mismatch",
      reportCommit: entry.commit,
      activeCommit: commit,
    });
    failed = true;
  }

  if (
    sourceFingerprint &&
    entry.sourceFingerprint &&
    entry.sourceFingerprint !== sourceFingerprint &&
    entry.sourceFingerprint !== "see-current-source-fingerprint" &&
    !String(entry.sourceFingerprint).startsWith("dirty-worktree")
  ) {
    findings.push({
      severity: "P1",
      path: req.path,
      status: "fingerprint_mismatch",
      reportFingerprint: entry.sourceFingerprint,
      activeFingerprint: sourceFingerprint,
    });
    failed = true;
  }

  if (
    typeof entry.sourceFingerprint === "string" &&
    (entry.sourceFingerprint.startsWith("dirty-worktree") ||
      entry.sourceFingerprint === "see-current-source-fingerprint")
  ) {
    findings.push({
      severity: "P1",
      path: req.path,
      status: "placeholder_fingerprint",
      fingerprint: entry.sourceFingerprint,
    });
    failed = true;
  }

  // Referenced nested evidence paths must exist when declared.
  for (const key of ["evidencePath", "reportPath", "artifactPath"]) {
    const ref = d[key];
    if (typeof ref === "string" && ref.startsWith("reports/")) {
      if (!fs.existsSync(path.join(root, ref))) {
        findings.push({
          severity: "P0",
          path: req.path,
          status: "references_missing_file",
          referenced: ref,
        });
        failed = true;
      }
    }
  }
  if (Array.isArray(d.journeys)) {
    for (const j of d.journeys) {
      const ref = j?.reportPath || j?.artifact || j?.evidencePath;
      if (typeof ref === "string" && ref.startsWith("reports/") && !fs.existsSync(path.join(root, ref))) {
        findings.push({
          severity: "P1",
          path: req.path,
          status: "journey_references_missing_file",
          journey: j?.id || j?.name,
          referenced: ref,
        });
        failed = true;
      }
    }
  }

  entries.push(entry);
}

for (const rule of OVERCLAIM_RULES) {
  const abs = path.join(root, rule.path);
  if (!fs.existsSync(abs)) continue;
  const parsed = readJson(abs);
  if (!parsed.ok) continue;
  const d = parsed.data;
  if (!rule.when(d)) continue;
  const level = d.evidenceLevel || d.classification || "";
  if (rule.forbidEvidenceLevels.includes(level)) {
    findings.push({
      severity: "P0",
      path: rule.path,
      status: "overclaim",
      evidenceLevel: level,
      detail: rule.reason,
    });
    failed = true;
  }
}

const generatedAt = new Date().toISOString();
const manifest = {
  schemaVersion: 1,
  product: "Coreside",
  generatedAt,
  generatedAtLocal: new Date().toString(),
  timezone: Intl.DateTimeFormat().resolvedOptions().timeZone || "unknown",
  commit,
  dirty,
  sourceFingerprint,
  packageLockHash,
  cargoLockHash,
  platform: process.platform,
  architecture: process.arch,
  buildMode: "audit",
  command: commandName,
  exitStatus: failed ? "failed" : "passed",
  evidenceLevel: failed ? "integrity-failed" : "integrity-validated",
  requiredCount: REQUIRED.length,
  presentCount: entries.filter((e) => e.presentOrMissing === "present").length,
  missingCount: entries.filter((e) => e.presentOrMissing === "missing").length,
  findings,
  entries,
};

fs.mkdirSync(reportsDir, { recursive: true });
fs.writeFileSync(outPath, JSON.stringify(manifest, null, 2) + "\n");

console.log(
  failed
    ? `evidence-manifest FAILED (${findings.length} finding(s)) → ${outPath}`
    : `evidence-manifest OK → ${outPath}`,
);
for (const f of findings.slice(0, 20)) {
  console.log(`  [${f.severity}] ${f.path}: ${f.status}${f.detail ? ` — ${f.detail}` : ""}`);
}
process.exit(failed ? 1 : 0);
