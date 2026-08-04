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
  if (
    d.exitStatus === "passed_partial" ||
    d.status === "partial" ||
    d.status === "partial_implementation" ||
    d.result === "partial" ||
    (typeof d.command === "string" && d.command.startsWith("manual-") &&
      (d.publicBeta === "Public beta ready" || d.verdict === "Public beta ready"))
  ) {
    findings.push({ severity: "P1", file, status: "partial_rejected" });
    failed = true;
  }
}

// Authority / ACL inventories: require generated evidence; reject manual/partial/missing.
const FORBIDDEN_MAIN_CREATE = [
  "core:window:allow-create",
  "core:webview:allow-create-webview-window",
];
let rawInvokePending = false;
for (const file of [
  "effective-window-authority.json",
  "tauri-capability-inventory.json",
]) {
  const r = readJson(file);
  if (r.missing) {
    findings.push({ severity: "P1", file, status: "missing" });
    failed = true;
    continue;
  }
  if (r.parseError || !r.data) {
    findings.push({
      severity: "P1",
      file,
      status: "invalid_json",
      detail: r.parseError || "empty",
    });
    failed = true;
    continue;
  }
  const d = r.data;
  if (
    d.status === "partial" ||
    d.status === "partial_implementation" ||
    d.status === "policy_violation" ||
    (typeof d.command === "string" && d.command.startsWith("manual-"))
  ) {
    findings.push({
      severity: "P1",
      file,
      status: "manual_or_partial_authority_rejected",
      detail: d.status || d.command || "manual/partial",
    });
    failed = true;
  } else if (d.status !== "generated_from_capabilities") {
    findings.push({
      severity: "P1",
      file,
      status: "authority_not_generated",
      detail: d.status || "missing_status",
    });
    failed = true;
  }
  if (d.rawInvokeDenialTest === "not_run") rawInvokePending = true;

  const main = d.windows?.main;
  if (main) {
    const coreList = [
      ...(Array.isArray(main.coreAndPluginAllows) ? main.coreAndPluginAllows : []),
      ...(Array.isArray(main.permissions) ? main.permissions : []),
    ];
    for (const perm of FORBIDDEN_MAIN_CREATE) {
      if (coreList.includes(perm)) {
        findings.push({
          severity: "P1",
          file,
          status: "main_window_create_permission_present",
          detail: perm,
        });
        failed = true;
      }
    }
  }
}
// E2E journey 11 evidence (raw-invoke denial). Missing/not_run keeps the ACL gate failed.
{
  const denial = readJson("command-authority-results.json");
  if (denial.missing) {
    rawInvokePending = true;
  } else if (denial.parseError || !denial.data) {
    findings.push({
      severity: "P1",
      file: "command-authority-results.json",
      status: "invalid_json",
      detail: denial.parseError || "empty",
    });
    failed = true;
    rawInvokePending = true;
  } else {
    const d = denial.data;
    if (
      d.status === "partial" ||
      d.status === "partial_implementation" ||
      (typeof d.command === "string" && d.command.startsWith("manual-"))
    ) {
      findings.push({
        severity: "P1",
        file: "command-authority-results.json",
        status: "manual_or_partial_authority_rejected",
        detail: d.status || d.command || "manual/partial",
      });
      failed = true;
      rawInvokePending = true;
    } else if (d.rawInvokeDenialTest === "passed" && d.status === "generated_from_e2e") {
      if (d.publicBeta === "Public beta ready") {
        findings.push({
          severity: "P1",
          file: "command-authority-results.json",
          status: "public_beta_ready_rejected",
          detail: "Journey 11 evidence must remain Not ready",
        });
        failed = true;
        rawInvokePending = true;
      } else if (
        typeof d.commit !== "string" ||
        !d.commit ||
        typeof d.dirty !== "boolean" ||
        d.publicBeta !== "Not ready"
      ) {
        findings.push({
          severity: "P1",
          file: "command-authority-results.json",
          status: "journey11_identity_incomplete",
          detail: "Requires commit, dirty, and publicBeta Not ready",
        });
        failed = true;
        // Evidence exists but is incomplete — not the same as not_run.
        rawInvokePending = false;
      } else if (d.commit !== commit) {
        findings.push({
          severity: "P1",
          file: "command-authority-results.json",
          status: "stale_commit",
          reportCommit: d.commit,
          activeCommit: commit,
        });
        failed = true;
        rawInvokePending = false;
      } else if (
        !Array.isArray(d.denials) ||
        d.denials.length === 0 ||
        d.denials.some((row) => !row || row.denied !== true)
      ) {
        findings.push({
          severity: "P1",
          file: "command-authority-results.json",
          status: "raw_invoke_denial_incomplete",
          detail: "Denial evidence missing or incomplete",
        });
        failed = true;
        rawInvokePending = true;
      } else if (d.crossToolStateWriteDenied !== true) {
        findings.push({
          severity: "P1",
          file: "command-authority-results.json",
          status: "cross_tool_write_not_denied",
        });
        failed = true;
        rawInvokePending = true;
      } else {
        rawInvokePending = false;
      }
    } else {
      rawInvokePending = true;
    }
  }
}
if (rawInvokePending) {
  findings.push({
    severity: "P1",
    file: "command-authority-results.json",
    status: "raw_invoke_denial_not_run",
  });
  failed = true;
}

const baseline = readJson("current-source-baseline.json");
if (!baseline.missing && baseline.data) {
  const beta = baseline.data.publicBeta;
  if (dirty && beta && beta !== "Not ready") {
    findings.push({
      severity: "P1",
      file: "current-source-baseline.json",
      status: "dirty_tree_cannot_claim_beta",
      detail: beta,
    });
    failed = true;
  }
  if (beta === "Public beta ready") {
    findings.push({
      severity: "P1",
      file: "current-source-baseline.json",
      status: "public_beta_ready_rejected",
      detail: "RC3 assurance must not accept Public beta ready",
    });
    failed = true;
  }
  // Corrupted porcelain paths (known off-by-one: "docs/..." → "ocs/...").
  const staged = baseline.data.stagedFiles;
  if (Array.isArray(staged)) {
    for (const p of staged) {
      if (typeof p === "string" && p.startsWith("ocs/")) {
        findings.push({
          severity: "P1",
          file: "current-source-baseline.json",
          status: "corrupt_staged_path",
          detail: p,
        });
        failed = true;
      }
    }
  }
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

// Evidence manifest integrity (required current reports + overclaim rules).
{
  const manifest = readJson("evidence-manifest.json");
  if (manifest.missing) {
    findings.push({
      severity: "P1",
      file: "evidence-manifest.json",
      status: "missing",
      detail: "Run npm run audit:evidence-manifest",
    });
    failed = true;
  } else if (manifest.parseError) {
    findings.push({
      severity: "P1",
      file: "evidence-manifest.json",
      status: "invalid_json",
      detail: manifest.parseError,
    });
    failed = true;
  } else if (manifest.data?.exitStatus === "failed") {
    findings.push({
      severity: "P1",
      file: "evidence-manifest.json",
      status: "integrity_failed",
      detail: `findings=${Array.isArray(manifest.data.findings) ? manifest.data.findings.length : "?"}`,
    });
    failed = true;
  }
}

// E2E honesty: Journey 7 (or any) passed_partial must not inflate suite status/evidenceLevel.
{
  const e2e = readJson("e2e-results.json");
  if (!e2e.missing && !e2e.parseError && e2e.data) {
    const d = e2e.data;
    const journeys = Array.isArray(d.journeys) ? d.journeys : [];
    const hasPartial = journeys.some((j) => j && j.status === "passed_partial");
    if (hasPartial && d.status === "passed") {
      findings.push({
        severity: "P1",
        file: "e2e-results.json",
        status: "passed_partial_inflated_to_passed",
        detail: "Suite status must be passed_partial when any journey is passed_partial",
      });
      failed = true;
    }
    if (hasPartial && d.evidenceLevel === "Desktop Verified") {
      findings.push({
        severity: "P1",
        file: "e2e-results.json",
        status: "partial_suite_cannot_claim_desktop_verified",
        detail: "Journey 7 passed_partial blocks Desktop Verified for the suite",
      });
      failed = true;
    }
  }
}

// Launch-only packaged smoke must not claim Packaged Verified.
{
  const smoke = readJson("packaged-smoke-results.json");
  if (!smoke.missing && !smoke.parseError && smoke.data) {
    const d = smoke.data;
    if (
      d.evidenceLevel === "Packaged Verified" &&
      (d.status === "launch_passed" ||
        d.reason === "artifact_scanned_and_launched")
    ) {
      findings.push({
        severity: "P1",
        file: "packaged-smoke-results.json",
        status: "launch_only_cannot_claim_packaged_verified",
        detail: "Brief launch/quit + scan is not Packaged Verified",
      });
      failed = true;
    }
  }
}

const report = {
  schemaVersion: 1,
  product: "Coreside",
  generatedAt: new Date().toISOString(),
  commit,
  dirty,
  command: "assurance:report",
  exitStatus: failed ? "failed" : "passed",
  narrowAssuranceOnly: true,
  fullReleasePass: false,
  note:
    "Narrow evidence validator only (presence/freshness/partial rejection). Not a full release pass. Public beta remains Not ready.",
  findings,
  localFirstVerdict: "Not ready",
  hostedAiVerdict: "Not ready",
  publicBeta: "Not ready",
};

fs.mkdirSync(reportsDir, { recursive: true });
fs.writeFileSync(outPath, JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify({ exitStatus: report.exitStatus, findings: findings.length, outPath }, null, 2));
process.exit(failed ? 1 : 0);
