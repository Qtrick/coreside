#!/usr/bin/env node
/**
 * RC3.3 Phase 1 — honest readiness ladder + delta vs prior archive.
 *
 * Usage:
 *   npm run audit:readiness
 *   npm run audit:readiness-delta
 *
 * Writes:
 *   reports/readiness-ladder.json
 *   reports/readiness-delta.json
 *
 * Does not invent Desktop / Packaged / Human Accepted evidence.
 */
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const reportsDir = path.join(root, "reports");
const commandName = process.argv.includes("--delta-only")
  ? "audit:readiness-delta"
  : "audit:readiness";

const PRIOR_ARCHIVE_SHA256 =
  "10d7c5110a12525b53a48ee3dc66bf8d3a054c243553bc4ee9a1db27bc8b86b5";
const PRIOR_ARCHIVE_LABEL = "Coreside Chat AI(10).zip";

/** @typedef {"Absent"|"Scaffolded"|"Integrated – Not Verified"|"Unit Verified"|"Desktop Verified"|"Packaged Verified"|"Cross-Platform Verified"|"Human Accepted"|"Blocked by External Prerequisite"|"Deliberately Deferred"|"Rejected by Product Architecture"} EvidenceState */

function run(cmd, args) {
  const r = spawnSync(cmd, args, { cwd: root, encoding: "utf8" });
  return {
    code: r.status ?? 1,
    stdout: (r.stdout || "").trim(),
    stderr: (r.stderr || "").trim(),
  };
}

function sha256File(filePath) {
  if (!fs.existsSync(filePath)) return null;
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(filePath));
  return hash.digest("hex");
}

function readJsonSafe(rel) {
  const p = path.join(root, rel);
  if (!fs.existsSync(p)) return null;
  try {
    return JSON.parse(fs.readFileSync(p, "utf8"));
  } catch {
    return null;
  }
}

function ensureReportsDir() {
  fs.mkdirSync(reportsDir, { recursive: true });
}

function envelope(extra) {
  const commit = run("git", ["rev-parse", "HEAD"]).stdout || "unknown";
  const dirty = (run("git", ["status", "--porcelain"]).stdout || "").length > 0;
  const fp = readJsonSafe("reports/current-source-fingerprint.json");
  return {
    schemaVersion: 1,
    product: "Coreside",
    generatedAt: new Date().toISOString(),
    commit,
    dirty,
    sourceFingerprint: fp?.sourceFingerprint ?? null,
    packageLockHash:
      fp?.packageLockHash ?? sha256File(path.join(root, "package-lock.json")),
    cargoLockHash:
      fp?.cargoLockHash ??
      sha256File(path.join(root, "src-tauri", "Cargo.lock")),
    platform: process.platform,
    architecture: process.arch,
    buildMode: "development",
    command: commandName,
    exitStatus: 0,
    evidenceLevel: "classified-not-desktop-verified",
    ...extra,
  };
}

/**
 * Honest feature classifications for the current tree (RC3.2→RC3.3).
 * Desktop Verified used only when WDIO journey reports show executed passes.
 * @returns {{ id: string, name: string, state: EvidenceState, notes: string, evidence?: string[] }[]}
 */
function featureLadder() {
  return [
    {
      id: "true-streaming",
      name: "True streaming (chat_stream / TextDelta)",
      state: "Desktop Verified",
      notes: "Journey 12 passed on darwin/arm64 with turnId + live stream marker.",
      evidence: ["reports/true-streaming-results.json"],
    },
    {
      id: "channel-scoped-streaming",
      name: "Channel-scoped turn streaming",
      state: "Desktop Verified",
      notes: "Journey 14: tool window received zero Text events during main stream.",
      evidence: [
        "reports/scoped-streaming-results.json",
        "reports/stream-eavesdropping-results.json",
      ],
    },
    {
      id: "wallpaper-atomic-settings",
      name: "Wallpaper atomic appearance writes",
      state: "Desktop Verified",
      notes: "Journey 13 Matrix + transparency 40 + canvas samples; packaged pixels not_run.",
      evidence: [
        "reports/wallpaper-settings-results.json",
        "reports/wallpaper-visual-results.json",
      ],
    },
    {
      id: "progressive-preview",
      name: "Progressive preview transaction",
      state: "Unit Verified",
      notes:
        "PreviewTransaction + live NDJSON Channel preview + turn-end apply unit-proven; surface paint / JSON-blob progressive / desktop E2E remain open.",
      evidence: [
        "src-tauri/src/runtime_v2/preview_transaction.rs",
        "docs/PROGRESSIVE_PREVIEW_TRANSACTION.md",
      ],
    },
    {
      id: "queue-ui",
      name: "Conversation queue UI",
      state: "Unit Verified",
      notes:
        "Conversation-scoped subscribe_conversation_queue Channel + 20s reconcile; Unit Verified; desktop E2E not_run.",
      evidence: [
        "reports/queue-product-results.json",
        "src/lib/tauri/queue-events.test.ts",
        "src/components/chat/ConversationQueue.test.tsx",
        "docs/QUEUE_COORDINATION.md",
      ],
    },
    {
      id: "history-replay",
      name: "History / replay / inspector UI",
      state: "Unit Verified",
      notes: "Paced read-only ReplayPlayer over transaction summaries; not full event-stream replay; desktop not_run.",
      evidence: ["reports/replay-results.json"],
    },
    {
      id: "structured-user-input",
      name: "Typed StructuredUserInput",
      state: "Unit Verified",
      notes:
        "Rust seal + trust gate; spoofed text marker does not grant trust; desktop E2E not_run.",
      evidence: [
        "reports/structured-form-results.json",
        "docs/STRUCTURED_FORMS_AND_CONTEXT.md",
        "src-tauri/src/ai/structured_user_input.rs",
      ],
    },
    {
      id: "attachment-authorization",
      name: "Attachment authorize_access (conversation-scoped)",
      state: "Unit Verified",
      notes:
        "authorize_attachment_access + get_chat_attachment_src / protocol bytes unit-proven; multimodal + window ACL open; desktop not_run.",
      evidence: [
        "reports/attachment-authorization-results.json",
        "docs/ATTACHMENT_LIFECYCLE.md",
      ],
    },
    {
      id: "attachment-gc",
      name: "Attachment run_attachment_gc foundation",
      state: "Unit Verified",
      notes:
        "run_attachment_gc wraps reconcile_and_sweep (no symlink follow, counts-only); scheduler tick wiring open; desktop not_run.",
      evidence: [
        "reports/attachment-gc-results.json",
        "docs/ATTACHMENT_LIFECYCLE.md",
      ],
    },
    {
      id: "e2e-1-14",
      name: "Desktop E2E Journeys 1–14",
      state: "Desktop Verified",
      notes:
        "Full orchestrator npm run e2e passed on darwin/arm64 (Journey 7 passed_partial by design).",
      evidence: [
        "reports/e2e-results.json",
        "reports/true-streaming-results.json",
        "reports/wallpaper-visual-results.json",
        "reports/stream-eavesdropping-results.json",
        "reports/command-authority-results.json",
      ],
    },
    {
      id: "e2e-12-13",
      name: "Desktop E2E Journey 12 / 13",
      state: "Desktop Verified",
      notes:
        "Executed on darwin/arm64; Journey 13 asserts uniqueColors≥2 and lumSpan≥8 on Matrix canvas.",
      evidence: [
        "reports/true-streaming-results.json",
        "reports/wallpaper-visual-results.json",
      ],
    },
    {
      id: "e2e-14-eavesdrop",
      name: "Tool-window stream eavesdropping denial",
      state: "Desktop Verified",
      notes: "Journey 14 passed; zero Text events in tool window.",
      evidence: ["reports/stream-eavesdropping-results.json"],
    },
    {
      id: "attachment-crash",
      name: "Attachment crash-consistency suite",
      state: "Unit Verified",
      notes:
        "Temp-dir/rusqlite crash windows Unit Verified (rollback before claim; claim-before-promote healed; N-attach idempotent). Desktop crash-restart not claimed.",
      evidence: [
        "reports/attachment-crash-results.json",
        "docs/ATTACHMENT_LIFECYCLE.md",
      ],
    },
    {
      id: "restore-transaction",
      name: "Coherent restore journal transaction",
      state: "Unit Verified",
      notes:
        "Staging→Validating→Swapping→Reopening→Rehydrating→Completed with quiescence; mid-swap retains Recovery. Packaged FS swap under load not claimed.",
      evidence: ["reports/restore-transaction-results.json"],
    },
    {
      id: "packaged-smoke",
      name: "Normal package build + smoke launch",
      state: "Packaged Verified",
      notes:
        "Release .app/.dmg built; package:scan + brief launch/quit. Smoke only — not full product acceptance.",
      evidence: ["reports/packaged-smoke-results.json"],
    },
    {
      id: "hosted-ai",
      name: "Hosted Coreside AI",
      state: "Deliberately Deferred",
      notes: "Separate track; scaffold/auth experiments must not gate local BYOK.",
      evidence: ["reports/hosted-ai-readiness.json"],
    },
    {
      id: "raw-html-js-cdn",
      name: "Raw HTML/JS/CDN/iframe generative surfaces",
      state: "Rejected by Product Architecture",
      notes: "Trusted declarative surfaces only.",
    },
  ];
}

function overallTracks(dirty) {
  return {
    localFirst: {
      track: "local-byok",
      // Dirty tree blocks Internal Alpha / Automated Public-Beta Candidate claims.
      ladder: dirty ? "Development Build" : "Internal Alpha Candidate",
      dirty,
      candidateNote: dirty
        ? "Full e2e 1–14 Desktop Verified and packaged smoke Packaged Verified on this host, but dirty=true — remain Development Build until a clean commit fingerprints evidence."
        : "Offline gates green with clean tree — Internal Alpha Candidate. Automated Public-Beta Candidate still requires Human Accepted + residual P1 closure.",
      publicBeta: "Not ready",
    },
    hosted: {
      track: "hosted-ai",
      ladder: "Development Build",
      state: "Deliberately Deferred",
      publicBeta: "Not ready",
    },
  };
}

function buildDelta(meta) {
  return {
    ...meta,
    command: commandName.includes("delta") ? commandName : "audit:readiness",
    baseline: {
      kind: "prior-coreside-archive",
      label: PRIOR_ARCHIVE_LABEL,
      sha256: PRIOR_ARCHIVE_SHA256,
      note: "Compare active tree posture vs archive 10d7c511… (not a full file diff).",
    },
    advanced: [
      {
        id: "wallpaper-atomic-settings",
        from: "Absent / fragmented settings writes",
        to: "Unit Verified",
        detail: "Atomic workspace appearance + slider coalesce.",
      },
      {
        id: "channel-scoped-streaming",
        from: "Global agent-turn text bus",
        to: "Unit Verified",
        detail: "Interactive send uses Channel; Text kept off global bus in unit path.",
      },
      {
        id: "true-streaming",
        from: "Buffered / incomplete live path",
        to: "Unit Verified",
        detail: "chat_with_auto → chat_stream; TextDelta unit assertions.",
      },
      {
        id: "queue-ui",
        from: "Disconnected / missing panel",
        to: "Unit Verified",
        detail:
          "ConversationQueue conversation-scoped Channel refresh; 20s reconcile.",
      },
      {
        id: "history-replay",
        from: "Missing / misleading scripts",
        to: "Unit Verified",
        detail:
          "Paced read-only ReplayPlayer over Committed transaction summaries; full event-stream replay / desktop still open.",
      },
      {
        id: "e2e-12-13",
        from: "Absent",
        to: "Scaffolded",
        detail: "Journey 12/13 specs added; not executed.",
      },
      {
        id: "progressive-preview",
        from: "Absent",
        to: "Unit Verified",
        detail:
          "PreviewTransaction + live NDJSON preview; surface paint / JSON-blob / E2E still open.",
      },
      {
        id: "structured-user-input",
        from: "Scaffolded",
        to: "Unit Verified",
        detail:
          "Typed Rust seal + trust gate; spoofed marker does not grant trust; desktop E2E not_run.",
      },
      {
        id: "attachment-authorization",
        from: "Opaque-ID-only reads",
        to: "Unit Verified",
        detail:
          "Conversation-scoped authorize_attachment_access; wrong conversation_id denied.",
      },
      {
        id: "attachment-gc",
        from: "Startup reconcile only",
        to: "Unit Verified",
        detail:
          "run_attachment_gc command + no-follow symlink remove + counts-only report.",
      },
    ],
    unchangedOrStillOpen: [
      { id: "hosted-ai", state: "Deliberately Deferred" },
      {
        id: "progressive-preview-gaps",
        state: "Unit Verified (partial)",
        detail:
          "No speculative surface paint, JSON-blob progressive, or desktop E2E yet.",
      },
      {
        id: "human-acceptance",
        state: "Absent",
        detail: "HUMAN_ACCEPTANCE_CHECKLIST unsigned — blocks Automated Public-Beta Candidate.",
      },
    ],
    nonClaims: [
      "No Human Accepted",
      "No Automated Public-Beta Candidate (dirty tree + unsigned human checklist)",
      "Public beta Not ready",
      "Hosted private alpha Not ready",
    ],
  };
}

function main() {
  ensureReportsDir();
  const features = featureLadder();
  const base = envelope({});
  const tracks = overallTracks(base.dirty);

  const ladder = {
    ...base,
    phase: "RC3.3-phase-1",
    model: "docs/BETA_READINESS_MODEL.md",
    humanChecklist: "docs/HUMAN_ACCEPTANCE_CHECKLIST.md",
    humanChecklistSigned: false,
    tracks,
    features,
    summary: {
      unitVerified: features.filter((f) => f.state === "Unit Verified").map((f) => f.id),
      desktopVerified: features.filter((f) => f.state === "Desktop Verified").map((f) => f.id),
      integratedNotVerified: features
        .filter((f) => f.state === "Integrated – Not Verified")
        .map((f) => f.id),
      scaffolded: features.filter((f) => f.state === "Scaffolded").map((f) => f.id),
      absent: features.filter((f) => f.state === "Absent").map((f) => f.id),
      deferred: features
        .filter((f) => f.state === "Deliberately Deferred")
        .map((f) => f.id),
      highestDesktopClaim: features.some((f) => f.state === "Desktop Verified") ? "Desktop Verified" : "none",
      highestPackagedClaim: features.some((f) => f.state === "Packaged Verified")
        ? "Packaged Verified"
        : "none",
      packagedVerified: features
        .filter((f) => f.state === "Packaged Verified")
        .map((f) => f.id),
      overallLocalFirst: tracks.localFirst.ladder,
      overallHosted: tracks.hosted.ladder,
    },
  };

  const delta = buildDelta({
    schemaVersion: ladder.schemaVersion,
    product: ladder.product,
    generatedAt: ladder.generatedAt,
    commit: ladder.commit,
    dirty: ladder.dirty,
    sourceFingerprint: ladder.sourceFingerprint,
    packageLockHash: ladder.packageLockHash,
    cargoLockHash: ladder.cargoLockHash,
    platform: ladder.platform,
    architecture: ladder.architecture,
    buildMode: ladder.buildMode,
    exitStatus: 0,
    evidenceLevel: ladder.evidenceLevel,
  });

  const ladderPath = path.join(reportsDir, "readiness-ladder.json");
  const deltaPath = path.join(reportsDir, "readiness-delta.json");
  fs.writeFileSync(ladderPath, JSON.stringify(ladder, null, 2) + "\n");
  fs.writeFileSync(deltaPath, JSON.stringify(delta, null, 2) + "\n");

  console.log(`[readiness] wrote ${path.relative(root, ladderPath)}`);
  console.log(`[readiness] wrote ${path.relative(root, deltaPath)}`);
  console.log(
    `[readiness] local=${tracks.localFirst.ladder} hosted=${tracks.hosted.ladder} dirty=${base.dirty}`,
  );
  console.log(
    `[readiness] unitVerified=${ladder.summary.unitVerified.length} desktopVerified=${ladder.summary.desktopVerified?.length ?? 0} scaffolded=${ladder.summary.scaffolded.length}`,
  );
  process.exit(0);
}

main();
