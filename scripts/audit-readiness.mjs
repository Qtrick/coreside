#!/usr/bin/env node
/**
 * RC3.6 — honest readiness ladder + delta vs prior archive.
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
 * Journey 7 passed_partial never upgrades e2e-1–14 to Desktop Verified.
 * Launch-only packaged smoke never mints Packaged Verified.
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

function pathExists(rel) {
  return fs.existsSync(path.join(root, rel));
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

/** Evidence may claim Desktop/Packaged Verified only when report commit matches ladder HEAD. */
function reportMatchesCommit(report, headCommit) {
  return (
    report &&
    typeof report.commit === "string" &&
    report.commit.length > 0 &&
    report.commit === headCommit
  );
}

function reportMatchesFingerprint(report, activeFingerprint) {
  const reportFp =
    (typeof report?.sourceFingerprint === "string" && report.sourceFingerprint) ||
    (typeof report?.fingerprint === "string" && report.fingerprint) ||
    null;
  return (
    typeof activeFingerprint === "string" &&
    activeFingerprint.length > 0 &&
    typeof reportFp === "string" &&
    reportFp === activeFingerprint
  );
}

function unitFeature(id, name, notes, evidence) {
  const missing = evidence.filter((e) => !pathExists(e));
  if (missing.length > 0) {
    return {
      id,
      name,
      state: /** @type {EvidenceState} */ ("Integrated – Not Verified"),
      notes: `${notes} Missing evidence paths: ${missing.join(", ")}.`,
      evidence,
    };
  }
  return {
    id,
    name,
    state: /** @type {EvidenceState} */ ("Unit Verified"),
    notes,
    evidence,
  };
}

/**
 * Classify a desktop journey feature from a results report.
 * Stale commit/fingerprint or missing file → not Desktop Verified.
 * @returns {{ state: EvidenceState, notes: string }}
 */
function desktopClaim(
  rel,
  report,
  passed,
  passNote,
  staleOrFailNote,
  headCommit,
  activeFingerprint,
) {
  if (!pathExists(rel)) {
    return {
      state: "Integrated – Not Verified",
      notes: `${staleOrFailNote} Evidence file missing (${rel}).`,
    };
  }
  if (!reportMatchesCommit(report, headCommit)) {
    const prior = report?.commit
      ? ` (prior evidence ${String(report.commit).slice(0, 7)})`
      : "";
    return {
      state: "Integrated – Not Verified",
      notes: `${staleOrFailNote}${prior} Re-run on HEAD to claim Desktop Verified.`,
    };
  }
  if (!reportMatchesFingerprint(report, activeFingerprint)) {
    const priorFp =
      report?.sourceFingerprint || report?.fingerprint
        ? ` (prior fingerprint ${String(report.sourceFingerprint || report.fingerprint).slice(0, 12)}…)`
        : "";
    return {
      state: "Integrated – Not Verified",
      notes: `${staleOrFailNote}${priorFp} Re-run on current source fingerprint to claim Desktop Verified.`,
    };
  }
  if (!passed) {
    return {
      state: "Integrated – Not Verified",
      notes: staleOrFailNote,
    };
  }
  return { state: "Desktop Verified", notes: passNote };
}

/**
 * Honest feature classifications for the current tree (RC3.6).
 * Desktop / Packaged Verified only when matching artifacts exist on HEAD commit.
 * @param {string} headCommit
 * @returns {{ id: string, name: string, state: EvidenceState, notes: string, evidence?: string[] }[]}
 */
function featureLadder(headCommit, activeFingerprint) {
  const e2e = readJsonSafe("reports/e2e-results.json");
  const packaged = readJsonSafe("reports/packaged-smoke-results.json");
  const trueStream = readJsonSafe("reports/true-streaming-results.json");
  const eavesdrop = readJsonSafe("reports/stream-eavesdropping-results.json");
  const wallpaperVisual = readJsonSafe("reports/wallpaper-visual-results.json");
  const wallpaperSettings = readJsonSafe("reports/wallpaper-settings-results.json");
  const scoped = readJsonSafe("reports/scoped-streaming-results.json");
  const commandAuthority = readJsonSafe("reports/command-authority-results.json");
  const multimodal = readJsonSafe("reports/multimodal-provider-results.json");

  const e2eFresh = reportMatchesCommit(e2e, headCommit);
  const e2eFingerprintFresh =
    e2eFresh && reportMatchesFingerprint(e2e, activeFingerprint);
  const e2eJourneys = Array.isArray(e2e?.journeys) ? e2e.journeys : [];
  const journey = (n) => e2eJourneys.find((j) => j.id === n);
  const journeyPassed = (n) => journey(n)?.status === "passed";
  const journey7Partial =
    journey(7)?.status === "passed_partial" || e2e?.status === "passed_partial";
  const e2eFullPass =
    e2eFingerprintFresh && e2e?.status === "passed" && !journey7Partial;

  // Prefer fresh e2e-results journey rows; fall back to dedicated reports on HEAD.
  const streamingPassedOnE2e = e2eFresh && journeyPassed(12);
  const streamingState =
    e2eFingerprintFresh && journeyPassed(12)
      ? {
          state: /** @type {EvidenceState} */ ("Desktop Verified"),
          notes:
            "Journey 12 passed on this HEAD (e2e-results) with turnId + live stream marker.",
        }
      : desktopClaim(
          "reports/e2e-results.json",
          e2e,
          streamingPassedOnE2e,
          "",
          streamingPassedOnE2e
            ? "Journey 12 passed in e2e-results but fingerprint stale — re-run e2e to claim Desktop Verified."
            : "True streaming desktop claim withheld.",
          headCommit,
          activeFingerprint,
        );

  const scopedPassedOnE2e = e2eFresh && journeyPassed(14);
  const scopedState =
    e2eFingerprintFresh && journeyPassed(14)
      ? {
          state: /** @type {EvidenceState} */ ("Desktop Verified"),
          notes:
            "Journey 14: tool window received zero Text events during main stream on this HEAD (e2e-results).",
        }
      : desktopClaim(
          "reports/e2e-results.json",
          e2e,
          scopedPassedOnE2e,
          "",
          scopedPassedOnE2e
            ? "Journey 14 passed in e2e-results but fingerprint stale — re-run e2e to claim Desktop Verified."
            : "Channel-scoped streaming desktop claim withheld.",
          headCommit,
          activeFingerprint,
        );

  const wallpaperPassedOnE2e = e2eFresh && journeyPassed(13);
  const wallpaperState =
    e2eFingerprintFresh && journeyPassed(13)
      ? {
          state: /** @type {EvidenceState} */ ("Desktop Verified"),
          notes:
            "Journey 13 Matrix + transparency + canvas samples on this HEAD (e2e-results); packaged pixels not_run.",
        }
      : desktopClaim(
          "reports/e2e-results.json",
          e2e,
          wallpaperPassedOnE2e,
          "",
          wallpaperPassedOnE2e
            ? "Journey 13 passed in e2e-results but fingerprint stale — re-run e2e to claim Desktop Verified."
            : "Wallpaper atomic settings desktop claim withheld.",
          headCommit,
          activeFingerprint,
        );

  const e2e114Evidence = [
    "reports/e2e-results.json",
    "reports/true-streaming-results.json",
    "reports/wallpaper-visual-results.json",
    "reports/stream-eavesdropping-results.json",
    "reports/command-authority-results.json",
  ];
  let e2e114State;
  let e2e114Notes;
  if (!pathExists("reports/e2e-results.json")) {
    e2e114State = "Integrated – Not Verified";
    e2e114Notes =
      "e2e-results.json missing — cannot claim Desktop Verified for journeys 1–14.";
  } else if (journey7Partial) {
    e2e114State = "Integrated – Not Verified";
    e2e114Notes = e2eFresh
      ? "Orchestrator finished on this HEAD but Journey 7 is passed_partial — not a full Desktop Verified suite pass."
      : `Journey 7 is passed_partial (evidence commit ${String(e2e?.commit || "unknown").slice(0, 7)}); suite must not be treated as Desktop Verified.`;
  } else if (e2eFullPass) {
    e2e114State = "Desktop Verified";
    e2e114Notes =
      "Full orchestrator npm run e2e passed on this HEAD with no passed_partial journeys.";
  } else if (!e2eFresh) {
    e2e114State = "Integrated – Not Verified";
    e2e114Notes = `e2e-results.json is on ${String(e2e?.commit || "unknown").slice(0, 7)}, not HEAD — re-run e2e to claim Desktop Verified.`;
  } else {
    e2e114State = "Integrated – Not Verified";
    e2e114Notes = `e2e status=${e2e?.status ?? "unknown"} — not a full Desktop Verified suite pass.`;
  }

  const e21213PassedOnE2e =
    e2eFresh && journeyPassed(12) && journeyPassed(13);
  const e21213State =
    e2eFingerprintFresh && journeyPassed(12) && journeyPassed(13)
      ? {
          state: /** @type {EvidenceState} */ ("Desktop Verified"),
          notes:
            "Journeys 12/13 passed on this HEAD (e2e-results); Journey 13 asserts uniqueColors≥2 and lumSpan≥8 on Matrix canvas.",
        }
      : desktopClaim(
          "reports/e2e-results.json",
          e2e,
          e21213PassedOnE2e,
          "",
          e21213PassedOnE2e
            ? "Journeys 12/13 passed in e2e-results but fingerprint stale — re-run e2e to claim Desktop Verified."
            : e2eFresh
              ? "Journey 12/13 not both passed on this HEAD."
              : "Journey 12/13 evidence not on HEAD.",
          headCommit,
          activeFingerprint,
        );

  const e214PassedOnE2e = e2eFresh && journeyPassed(14);
  const e214State =
    e2eFingerprintFresh && journeyPassed(14)
      ? {
          state: /** @type {EvidenceState} */ ("Desktop Verified"),
          notes:
            "Journey 14 passed on this HEAD (e2e-results); zero Text events in tool window.",
        }
      : desktopClaim(
          "reports/e2e-results.json",
          e2e,
          e214PassedOnE2e,
          "",
          e214PassedOnE2e
            ? "Journey 14 passed in e2e-results but fingerprint stale — re-run e2e to claim Desktop Verified."
            : "Journey 14 eavesdrop desktop claim withheld.",
          headCommit,
          activeFingerprint,
        );

  const journey11State =
    commandAuthority?.rawInvokeDenialTest === "passed" &&
    commandAuthority?.status === "generated_from_e2e" &&
    commandAuthority?.isolatedSuiteOnly === true &&
    commandAuthority?.evidenceLevel === "Isolated Desktop Verified" &&
    reportMatchesCommit(commandAuthority, headCommit) &&
    reportMatchesFingerprint(commandAuthority, activeFingerprint)
      ? {
          state: /** @type {EvidenceState} */ ("Desktop Verified"),
          notes:
            "Journey 11 raw-invoke denial passed on this HEAD (isolated suite only; 15 sensitive commands + cross-tool write denied). Full orchestrator passed_partial (J7) — not a Desktop Verified suite pass.",
        }
      : desktopClaim(
          "reports/command-authority-results.json",
          commandAuthority,
          false,
          "",
          "Journey 11 command-authority desktop claim withheld.",
          headCommit,
          activeFingerprint,
        );

  // Packaged: never mint Packaged Verified from launch-only smoke.
  let packagedState;
  let packagedNotes;
  const packagedEvidence = ["reports/packaged-smoke-results.json"];
  if (!pathExists("reports/packaged-smoke-results.json")) {
    packagedState = "Scaffolded";
    packagedNotes =
      "packaged-smoke-results.json missing — run npm run test:packaged-smoke after build.";
  } else if (
    packaged?.status === "launch_passed" ||
    packaged?.evidenceLevel === "Packaged Verified" ||
    packaged?.evidenceLevel === "Integrated – Not Verified"
  ) {
    packagedState = "Integrated – Not Verified";
    packagedNotes = reportMatchesCommit(packaged, headCommit)
      ? "Release .app/.dmg scanned + brief launch/quit on this HEAD. Launch-only smoke — not Packaged Verified."
      : `Launch-only smoke recorded on ${String(packaged?.commit || "unknown").slice(0, 7)} (not HEAD). Not Packaged Verified.`;
  } else {
    packagedState = "Scaffolded";
    packagedNotes = `Packaged smoke status=${packaged?.status ?? "unknown"} — not Packaged Verified.`;
  }

  return [
    {
      id: "true-streaming",
      name: "True streaming (chat_stream / TextDelta)",
      state: streamingState.state,
      notes: streamingState.notes,
      evidence: ["reports/true-streaming-results.json"],
    },
    {
      id: "channel-scoped-streaming",
      name: "Channel-scoped turn streaming",
      state: scopedState.state,
      notes: scopedState.notes,
      evidence: [
        "reports/scoped-streaming-results.json",
        "reports/stream-eavesdropping-results.json",
      ],
    },
    {
      id: "wallpaper-atomic-settings",
      name: "Wallpaper atomic appearance writes",
      state: wallpaperState.state,
      notes: wallpaperState.notes,
      evidence: [
        "reports/wallpaper-settings-results.json",
        "reports/wallpaper-visual-results.json",
      ],
    },
    unitFeature(
      "progressive-preview",
      "Progressive preview transaction",
      "PreviewTransaction + live NDJSON Channel preview + speculative surface paint unit-proven; JSON-blob progressive / desktop E2E remain open.",
      [
        "src-tauri/src/runtime_v2/preview_transaction.rs",
        "docs/PROGRESSIVE_PREVIEW_TRANSACTION.md",
      ],
    ),
    unitFeature(
      "queue-ui",
      "Conversation queue UI",
      "Conversation-scoped subscribe_conversation_queue Channel + 20s reconcile; Unit Verified; desktop E2E not_run.",
      [
        "reports/queue-product-results.json",
        "src/lib/tauri/queue-events.test.ts",
        "src/components/chat/ConversationQueue.test.tsx",
        "docs/QUEUE_COORDINATION.md",
      ],
    ),
    unitFeature(
      "history-replay",
      "History / replay / inspector UI",
      "Paced read-only ReplayPlayer over transaction summaries; not full event-stream replay; desktop not_run.",
      ["reports/replay-results.json"],
    ),
    unitFeature(
      "structured-user-input",
      "Typed StructuredUserInput",
      "Rust seal + trust gate; spoofed text marker does not grant trust; desktop E2E not_run.",
      [
        "reports/structured-form-results.json",
        "docs/STRUCTURED_FORMS_AND_CONTEXT.md",
        "src-tauri/src/ai/structured_user_input.rs",
      ],
    ),
    unitFeature(
      "attachment-authorization",
      "Attachment authorize_access (conversation-scoped)",
      "authorize_attachment_access + get_chat_attachment_src / protocol bytes unit-proven; multimodal send path wired; desktop E2E not_run.",
      [
        "reports/attachment-authorization-results.json",
        "docs/ATTACHMENT_LIFECYCLE.md",
      ],
    ),
    unitFeature(
      "attachment-gc",
      "Attachment run_attachment_gc foundation",
      "run_attachment_gc wraps reconcile_and_sweep (no symlink follow, counts-only); scheduler tick wiring open; desktop not_run.",
      ["reports/attachment-gc-results.json", "docs/ATTACHMENT_LIFECYCLE.md"],
    ),
    {
      id: "e2e-1-14",
      name: "Desktop E2E Journeys 1–14",
      state: /** @type {EvidenceState} */ (e2e114State),
      notes: e2e114Notes,
      evidence: e2e114Evidence,
    },
    {
      id: "e2e-12-13",
      name: "Desktop E2E Journey 12 / 13",
      state: e21213State.state,
      notes: e21213State.notes,
      evidence: [
        "reports/true-streaming-results.json",
        "reports/wallpaper-visual-results.json",
      ],
    },
    {
      id: "e2e-14-eavesdrop",
      name: "Tool-window stream eavesdropping denial",
      state: e214State.state,
      notes: e214State.notes,
      evidence: ["reports/stream-eavesdropping-results.json"],
    },
    {
      id: "command-authority-j11",
      name: "Tool-window raw-invoke ACL denial (Journey 11)",
      state: journey11State.state,
      notes: journey11State.notes,
      evidence: ["reports/command-authority-results.json"],
    },
    unitFeature(
      "multimodal-provider-send",
      "Multimodal image + native tool-result send path",
      "OpenAI/Ollama/Anthropic/Gemini family adapters + validate_provider_send unit-proven; Desktop E2E multimodal chat not_run.",
      [
        "reports/multimodal-provider-results.json",
        "docs/ATTACHMENT_LIFECYCLE.md",
        "src-tauri/src/ai/provider_send.rs",
      ],
    ),
    {
      id: "local-ai-privacy",
      name: "Local AI privacy routing (user_local)",
      state: pathExists("e2e/specs/17-local-ai-privacy.spec.ts")
        ? /** @type {EvidenceState} */ ("Integrated – Not Verified")
        : /** @type {EvidenceState} */ ("Absent"),
      notes:
        "access_mode.rs + Composer disclosure landed; Journey 17 spec registered; harness not_run (requires Local AI desktop profile).",
      evidence: [
        "src-tauri/src/ai/access_mode.rs",
        "src/lib/ai-access-disclosure.test.ts",
        "e2e/specs/17-local-ai-privacy.spec.ts",
      ],
    },
    {
      id: "hosted-ai-gateway",
      name: "Hosted Coreside AI gateway + billing scaffold",
      state: /** @type {EvidenceState} */ ("Scaffolded"),
      notes:
        "Supabase ai-gateway + billing Edge Functions + entitlements migrations unit-tested; private alpha not ready; separate track from local BYOK.",
      evidence: [
        "supabase/functions/ai-gateway/gateway-lib.test.ts",
        "supabase/functions/billing-lib.test.ts",
        "reports/hosted-ai-readiness.json",
      ],
    },
    unitFeature(
      "attachment-crash",
      "Attachment crash-consistency suite",
      "Temp-dir/rusqlite crash windows Unit Verified (rollback before claim; claim-before-promote healed; N-attach idempotent). Desktop crash-restart not claimed.",
      ["reports/attachment-crash-results.json", "docs/ATTACHMENT_LIFECYCLE.md"],
    ),
    unitFeature(
      "restore-transaction",
      "Coherent restore journal transaction",
      "Staging→Validating→Swapping→Reopening→Rehydrating→Completed with quiescence; mid-swap retains Recovery. Packaged FS swap under load not claimed.",
      ["reports/restore-transaction-results.json"],
    ),
    {
      id: "packaged-smoke",
      name: "Normal package build + smoke launch",
      state: /** @type {EvidenceState} */ (packagedState),
      notes: packagedNotes,
      evidence: packagedEvidence,
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

function overallTracks(dirty, features) {
  const hasDesktop = features.some((f) => f.state === "Desktop Verified");
  const hasPackaged = features.some((f) => f.state === "Packaged Verified");
  const e2e114 = features.find((f) => f.id === "e2e-1-14");
  const packaged = features.find((f) => f.id === "packaged-smoke");
  return {
    localFirst: {
      track: "local-byok",
      // Dirty tree blocks Internal Alpha / Automated Public-Beta Candidate claims.
      ladder: dirty ? "Development Build" : "Internal Alpha Candidate",
      dirty,
      candidateNote: dirty
        ? `Dirty tree — remain Development Build. e2e-1-14=${e2e114?.state ?? "unknown"}; packaged-smoke=${packaged?.state ?? "unknown"} (launch-only ≠ Packaged Verified).`
        : hasDesktop || hasPackaged
          ? "Clean tree with some desktop/packaged claims — Internal Alpha Candidate only when evidence is on HEAD. Automated Public-Beta Candidate still requires Human Accepted + residual P1 closure."
          : "Clean tree; desktop/packaged claims not on HEAD or intentionally partial — Internal Alpha Candidate pending fresh evidence. Public beta Not ready.",
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

function buildDelta(meta, features) {
  const byId = new Map(features.map((f) => [f.id, f]));
  const stateOf = (id, fallback) => byId.get(id)?.state ?? fallback;
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
        to: stateOf("wallpaper-atomic-settings", "Unit Verified"),
        detail:
          "Atomic workspace appearance + slider coalesce; desktop only when Journey 13 evidence is on HEAD.",
      },
      {
        id: "channel-scoped-streaming",
        from: "Global agent-turn text bus",
        to: stateOf("channel-scoped-streaming", "Unit Verified"),
        detail:
          "Interactive send uses Channel; Text kept off global bus; desktop when Journey 14 evidence is on HEAD.",
      },
      {
        id: "true-streaming",
        from: "Buffered / incomplete live path",
        to: stateOf("true-streaming", "Unit Verified"),
        detail:
          "chat_with_auto → chat_stream; desktop when Journey 12 evidence is on HEAD.",
      },
      {
        id: "queue-ui",
        from: "Disconnected / missing panel",
        to: stateOf("queue-ui", "Unit Verified"),
        detail:
          "ConversationQueue conversation-scoped Channel refresh; 20s reconcile.",
      },
      {
        id: "history-replay",
        from: "Missing / misleading scripts",
        to: stateOf("history-replay", "Unit Verified"),
        detail:
          "Paced read-only ReplayPlayer over Committed transaction summaries; full event-stream replay / desktop still open.",
      },
      {
        id: "e2e-12-13",
        from: "Absent",
        to: stateOf("e2e-12-13", "Integrated – Not Verified"),
        detail:
          "Journey 12/13 specs exist; Desktop Verified only when both pass on HEAD.",
      },
      {
        id: "progressive-preview",
        from: "Absent",
        to: stateOf("progressive-preview", "Unit Verified"),
        detail:
          "PreviewTransaction + live NDJSON preview; surface paint / JSON-blob / E2E still open.",
      },
      {
        id: "structured-user-input",
        from: "Scaffolded",
        to: stateOf("structured-user-input", "Unit Verified"),
        detail:
          "Typed Rust seal + trust gate; spoofed marker does not grant trust; desktop E2E not_run.",
      },
      {
        id: "attachment-authorization",
        from: "Opaque-ID-only reads",
        to: stateOf("attachment-authorization", "Unit Verified"),
        detail:
          "Conversation-scoped authorize_attachment_access; wrong conversation_id denied.",
      },
      {
        id: "attachment-gc",
        from: "Startup reconcile only",
        to: stateOf("attachment-gc", "Unit Verified"),
        detail:
          "run_attachment_gc command + no-follow symlink remove + counts-only report.",
      },
      {
        id: "packaged-smoke",
        from: "Absent",
        to: stateOf("packaged-smoke", "Integrated – Not Verified"),
        detail:
          "Launch-only scan+quit recorded when present — not Packaged Verified until BYOK/chat/persistence packaged proof.",
      },
      {
        id: "command-authority-j11",
        from: "Absent / manual",
        to: stateOf("command-authority-j11", "Integrated – Not Verified"),
        detail:
          "Journey 11 E2E denies 15 sensitive invokes + cross-tool save_tool_state from tool-* windows.",
      },
      {
        id: "multimodal-provider-send",
        from: "Text-only flatten",
        to: stateOf("multimodal-provider-send", "Unit Verified"),
        detail:
          "Native image + tool-result envelopes per provider family; validate_provider_send fail-closed.",
      },
      {
        id: "local-ai-privacy",
        from: "Absent",
        to: stateOf("local-ai-privacy", "Integrated – Not Verified"),
        detail:
          "Rust access_mode + UI disclosure; Journey 17 spec not executed in default harness.",
      },
      {
        id: "hosted-ai-gateway",
        from: "Absent",
        to: stateOf("hosted-ai-gateway", "Scaffolded"),
        detail:
          "ai-gateway + Stripe billing scaffold + entitlements SQL; hosted track Deliberately Deferred.",
      },
      {
        id: "e2e-1-14",
        from: "Absent",
        to: stateOf("e2e-1-14", "Integrated – Not Verified"),
        detail:
          "Journey 7 passed_partial blocks full Desktop Verified for the 1–14 suite.",
      },
    ],
    unchangedOrStillOpen: [
      { id: "hosted-ai", state: "Deliberately Deferred" },
      {
        id: "progressive-preview-gaps",
        state: "Unit Verified (partial)",
        detail:
          "Speculative surface paint + conversation-scoped overlay clear landed; JSON-blob progressive + desktop E2E still open.",
      },
      {
        id: "packaged-smoke-full",
        state: "Integrated – Not Verified",
        detail:
          "Launch-only smoke ≠ Packaged Verified; BYOK/chat/persistence packaged checklist still open.",
      },
      {
        id: "human-acceptance",
        state: "Absent",
        detail:
          "HUMAN_ACCEPTANCE_CHECKLIST unsigned — blocks Automated Public-Beta Candidate.",
      },
    ],
    nonClaims: [
      "No Human Accepted",
      "No Automated Public-Beta Candidate (unsigned human checklist; Journey 7 partial; launch-only packaged smoke)",
      "Public beta Not ready",
      "Hosted private alpha Not ready",
      "Journey 7 passed_partial is not a Desktop Verified suite pass",
      "Launch-only packaged smoke is not Packaged Verified",
    ],
  };
}

function summarizeHighestDesktopClaim(features) {
  const desktop = features.filter((f) => f.state === "Desktop Verified");
  if (desktop.length === 0) return "none";
  const ids = new Set(desktop.map((f) => f.id));
  if (ids.has("e2e-1-14")) return "Desktop Verified (journeys 1–14 suite)";
  if (ids.size === 1 && ids.has("command-authority-j11")) {
    return "Desktop Verified (isolated Journey 11 only)";
  }
  return "Desktop Verified (partial feature evidence)";
}

function main() {
  ensureReportsDir();
  const base = envelope({});
  const features = featureLadder(base.commit, base.sourceFingerprint);
  const tracks = overallTracks(base.dirty, features);

  const ladder = {
    ...base,
    phase: "RC3.6",
    model: "docs/BETA_READINESS_MODEL.md",
    humanChecklist: "docs/HUMAN_ACCEPTANCE_CHECKLIST.md",
    humanChecklistSigned: false,
    tracks,
    features,
    summary: {
      unitVerified: features.filter((f) => f.state === "Unit Verified").map((f) => f.id),
      desktopVerified: features
        .filter((f) => f.state === "Desktop Verified")
        .map((f) => f.id),
      integratedNotVerified: features
        .filter((f) => f.state === "Integrated – Not Verified")
        .map((f) => f.id),
      scaffolded: features.filter((f) => f.state === "Scaffolded").map((f) => f.id),
      absent: features.filter((f) => f.state === "Absent").map((f) => f.id),
      deferred: features
        .filter((f) => f.state === "Deliberately Deferred")
        .map((f) => f.id),
      highestDesktopClaim: summarizeHighestDesktopClaim(features),
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

  const delta = buildDelta(
    {
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
    },
    features,
  );

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
    `[readiness] unitVerified=${ladder.summary.unitVerified.length} desktopVerified=${ladder.summary.desktopVerified?.length ?? 0} integratedNotVerified=${ladder.summary.integratedNotVerified.length} packagedVerified=${ladder.summary.packagedVerified.length}`,
  );
  process.exit(0);
}

main();
