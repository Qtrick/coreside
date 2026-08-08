#!/usr/bin/env node
/**
 * Current-source Partial Update secure-parity audit.
 *
 * Extracts/reads the Partial Update archive, probes both trees with content
 * signals (not mere file existence), classifies each feature, and writes:
 *   - reports/partial-update-feature-matrix.json
 *   - reports/partial-update-current-source-audit-2026-08-07.md
 *
 * Usage: npm run audit:partial-update
 * Optional: PARTIAL_UPDATE_ARCHIVE=/path/to/Partial Update Main.zip
 */
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const reportsDir = path.join(root, "reports");
const artifactsDir = path.join(reportsDir, ".artifacts");
const ACCESS_DATE = "2026-08-07";
const commandName = "audit:partial-update";

const PARTIAL_UPDATE_ARCHIVE =
  process.env.PARTIAL_UPDATE_ARCHIVE ||
  path.join(process.env.HOME || "", "Downloads", "Partial Update Main.zip");
const EXPECTED_PARTIAL_UPDATE_SHA256 =
  "8666c226cb875deae8a73e6d2c7c09965f311b09c3db15ea1d1305261a3eb607";

/** Canonical evidence vocabulary (exact strings). */
const STATUS = Object.freeze({
  Absent: "Absent",
  Scaffolded: "Scaffolded",
  Implemented: "Implemented",
  UnitVerified: "Unit Verified",
  IntegratedNotVerified: "Integrated – Not Verified",
  DesktopVerified: "Desktop Verified",
  PackagedVerified: "Packaged Verified",
  CrossPlatformVerified: "Cross-Platform Verified",
  HumanAccepted: "Human Accepted",
  IntentionallyRejected: "Intentionally Rejected",
  Deferred: "Deferred",
});

function nowIso() {
  return new Date().toISOString();
}

function sha256File(filePath) {
  return crypto.createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
}

function run(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, {
    cwd: opts.cwd || root,
    encoding: "utf8",
    ...opts,
  });
  if (r.error) {
    const code = r.error.code || "spawn_error";
    return {
      status: code === "ENOENT" ? "missing_optional" : "fail",
      code: 1,
      stdout: "",
      stderr: String(r.error.message || r.error),
      errorCode: code,
    };
  }
  const exit = r.status ?? 1;
  return {
    status: exit === 0 ? "pass" : "fail",
    code: exit,
    stdout: (r.stdout || "").trim(),
    stderr: (r.stderr || "").trim(),
  };
}

function git(args) {
  const r = run("git", args);
  return r.status === "pass" ? r.stdout : "";
}

function readJson(absOrRel) {
  const abs = path.isAbsolute(absOrRel) ? absOrRel : path.join(root, absOrRel);
  if (!fs.existsSync(abs)) return null;
  try {
    return JSON.parse(fs.readFileSync(abs, "utf8"));
  } catch {
    return null;
  }
}

function readText(abs) {
  if (!fs.existsSync(abs)) return null;
  try {
    return fs.readFileSync(abs, "utf8");
  } catch {
    return null;
  }
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

/**
 * Resolve Partial Update source root: reuse matching extract, else unzip under
 * reports/.artifacts (small archive; cleaned marker for reuse).
 */
function resolvePartialUpdateRoot(archivePath, expectedSha) {
  const short = expectedSha.slice(0, 12);
  const artifactRoot = path.join(artifactsDir, `partial-update-${short}`);
  const marker = path.join(artifactRoot, "source.sha256");
  const nestedCandidates = [
    path.join(artifactRoot, "partialupdate-main"),
    path.join(artifactRoot, "Partial Update Main", "partialupdate-main"),
    artifactRoot,
  ];

  const referenceCandidates = [
    path.join(root, ".reference", "partial-update", "partialupdate-main"),
    path.join(root, ".reference", "partial-update-main", "partialupdate-main"),
  ];

  for (const ref of referenceCandidates) {
    const shaFile = path.join(path.dirname(ref), "source.sha256");
    const sha = readText(shaFile)?.trim();
    if (sha === expectedSha && fs.existsSync(path.join(ref, "src", "index.ts"))) {
      return {
        root: ref,
        source: "reference_reuse",
        artifactRoot: path.dirname(ref),
      };
    }
  }

  if (
    fs.existsSync(marker) &&
    readText(marker)?.trim() === expectedSha
  ) {
    for (const c of nestedCandidates) {
      if (fs.existsSync(path.join(c, "src", "index.ts"))) {
        return { root: c, source: "artifact_reuse", artifactRoot };
      }
    }
  }

  if (!fs.existsSync(archivePath)) {
    return { root: null, source: "archive_unavailable", artifactRoot };
  }

  ensureDir(artifactRoot);
  // Fresh extract into a clean child to avoid stale trees.
  const extractTmp = path.join(artifactRoot, "_extract");
  fs.rmSync(extractTmp, { recursive: true, force: true });
  ensureDir(extractTmp);
  const unzip = run("unzip", ["-qo", archivePath, "-d", extractTmp]);
  if (unzip.status !== "pass") {
    return {
      root: null,
      source: "extract_failed",
      artifactRoot,
      error: unzip.stderr || unzip.errorCode,
    };
  }

  let found = null;
  const walk = (dir, depth) => {
    if (found || depth > 4) return;
    if (fs.existsSync(path.join(dir, "src", "index.ts"))) {
      found = dir;
      return;
    }
    for (const name of fs.readdirSync(dir)) {
      const abs = path.join(dir, name);
      if (fs.statSync(abs).isDirectory()) walk(abs, depth + 1);
    }
  };
  walk(extractTmp, 0);

  if (!found) {
    return { root: null, source: "extract_missing_src", artifactRoot };
  }

  // Move to stable path partialupdate-main under artifactRoot.
  const dest = path.join(artifactRoot, "partialupdate-main");
  fs.rmSync(dest, { recursive: true, force: true });
  fs.renameSync(found, dest);
  fs.rmSync(extractTmp, { recursive: true, force: true });
  fs.writeFileSync(marker, `${expectedSha}\n`);

  return { root: dest, source: "extracted", artifactRoot };
}

function matchAll(text, patterns) {
  if (!text) return false;
  return patterns.every((p) => (p instanceof RegExp ? p.test(text) : text.includes(p)));
}

function matchAny(text, patterns) {
  if (!text) return false;
  return patterns.some((p) => (p instanceof RegExp ? p.test(text) : text.includes(p)));
}

function probeFiles(baseRoot, specs) {
  const hits = [];
  const misses = [];
  for (const spec of specs || []) {
    const abs = path.join(baseRoot, spec.path);
    const text = readText(abs);
    const ok =
      text != null &&
      (spec.mustMatch
        ? matchAll(text, spec.mustMatch)
        : spec.anyMatch
          ? matchAny(text, spec.anyMatch)
          : true);
    const entry = {
      path: spec.path,
      present: text != null,
      matched: Boolean(ok),
    };
    if (ok) hits.push(entry);
    else misses.push(entry);
  }
  return {
    ok: hits.length > 0 && misses.length === 0,
    any: hits.length > 0,
    hits,
    misses,
  };
}

function hasSubstantiveTest(relPath, hintPatterns) {
  const text = readText(path.join(root, relPath));
  if (!text) return false;
  const hasTestHarness =
    /#\[test\]|#\[tokio::test\]|describe\(|it\(|test\(/.test(text);
  const hasAssert = /assert!|assert_eq!|expect\(|toBe|toEqual|toMatch/.test(text);
  if (!hasTestHarness || !hasAssert) return false;
  if (!hintPatterns || hintPatterns.length === 0) return true;
  return matchAny(text, hintPatterns);
}

function reportEvidenceLevel(rel, { requireFreshFingerprint = false, fingerprint = null } = {}) {
  const report = readJson(rel);
  if (!report) return { level: null, fresh: false, report: null };
  const level =
    report.evidenceLevel ||
    report.result ||
    report.levels?.unit ||
    null;
  const reportFp =
    typeof report.sourceFingerprint === "string" ? report.sourceFingerprint : null;
  // Unknown current fingerprint ⇒ cannot prove freshness for Desktop/Packaged claims.
  let fresh = true;
  if (requireFreshFingerprint) {
    if (!fingerprint || !reportFp) fresh = false;
    else fresh = reportFp === fingerprint;
  }
  return { level, fresh, report };
}

function normalizeEvidenceLevel(raw) {
  if (!raw || typeof raw !== "string") return null;
  const t = raw.trim();
  if (Object.values(STATUS).includes(t)) return t;
  if (t === "passed" || t === "passed_unit" || t === "Unit Verified") return STATUS.UnitVerified;
  if (t === "Desktop Verified") return STATUS.DesktopVerified;
  if (t === "Packaged Verified") return STATUS.PackagedVerified;
  if (t === "deliberately-rejected" || t === "rejected_security") {
    return STATUS.IntentionallyRejected;
  }
  return null;
}

/**
 * Feature catalog — probes require content signals, not path existence alone.
 */
const FEATURES = [
  {
    id: "PU-STREAM",
    name: "Provider streaming (live deltas)",
    category: "streaming",
    userValue: 10,
    pu: [
      { path: "src/index.ts", mustMatch: [/UpdateStreamParser/, /runModel|stream/] },
    ],
    coresideImpl: [
      { path: "src-tauri/src/ai/provider.rs", mustMatch: [/async fn chat_stream/] },
      {
        path: "src-tauri/src/ai/openai.rs",
        mustMatch: [/consume_sse_chat_stream/, /stream:\s*true|\"stream\"/],
      },
      {
        path: "src-tauri/src/commands/message_cmds.rs",
        mustMatch: [/chat_with_auto/, /TextDelta|streamed_live|peek_assistant_message/],
      },
    ],
    coresideWired: [
      { path: "src-tauri/src/ai/auto.rs", mustMatch: [/chat_stream/] },
    ],
    unitFiles: [
      {
        path: "src-tauri/src/ai/mock.rs",
        hints: [/true_streaming|live stream probe|TextDelta/],
      },
    ],
    unitReports: ["reports/true-streaming-results.json", "reports/provider-conformance-results.json"],
    desktopReports: ["reports/true-streaming-results.json"],
    e2eHints: [/true.streaming|12-true-streaming/],
  },
  {
    id: "PU-PARSE",
    name: "NDJSON / UpdateStreamParser incremental parse",
    category: "protocol",
    userValue: 9,
    pu: [
      { path: "src/index.ts", mustMatch: [/class UpdateStreamParser/, /push\(/] },
    ],
    coresideImpl: [
      {
        path: "src-tauri/src/runtime_v2/streaming.rs",
        mustMatch: [/struct NdjsonFrameParser/, /fn push/],
      },
    ],
    coresideWired: [
      {
        path: "src-tauri/src/commands/message_cmds.rs",
        mustMatch: [/NdjsonFrameParser|ProgressiveOpsParser|ingest_live/],
      },
    ],
    unitFiles: [
      {
        path: "src-tauri/src/runtime_v2/streaming.rs",
        hints: [/NdjsonFrameParser/, /#\[test\]/],
      },
    ],
  },
  {
    id: "PU-PROGRESSIVE-PREVIEW",
    name: "Progressive generated-surface preview",
    category: "generative",
    userValue: 10,
    pu: [
      {
        path: "src/index.ts",
        mustMatch: [/UpdateStreamParser/, /broadcast|apply/],
      },
    ],
    coresideImpl: [
      {
        path: "src-tauri/src/runtime_v2/preview_transaction.rs",
        mustMatch: [/struct PreviewTransaction/, /ingest_live_chunk/],
      },
      {
        path: "src/lib/preview/surface-overlay.ts",
        mustMatch: [/applyPreviewSurfaceOverlay/],
      },
    ],
    coresideWired: [
      {
        path: "src-tauri/src/commands/message_cmds.rs",
        mustMatch: [/PreviewTransaction/, /PreviewSurface/],
      },
      {
        path: "src/stores/app-store.ts",
        mustMatch: [/applyPreviewSurfaceOverlay/, /previewSurfacesByKey/],
      },
      {
        path: "src/components/tool-canvas/ToolCanvas.tsx",
        mustMatch: [/previewSurfacesByKey/, /tool-preview-badge|Preview/],
      },
    ],
    unitFiles: [
      {
        path: "src-tauri/src/runtime_v2/preview_transaction.rs",
        hints: [
          /#\[test\]/,
          /PreviewTransaction|ingest_live/,
          /cancel_after_speculative_paint_leaves_sqlite_unchanged|incomplete_progressive_finish_rolls_back/,
        ],
      },
      {
        path: "src/lib/preview/surface-overlay.test.ts",
        hints: [/applyPreviewSurfaceOverlay/, /expect\(/],
      },
    ],
    // Honest gap: SQLite cancel/incomplete integration exists; desktop/packaged
    // progressive-surface paint journey still not_run (Journey 12 is text-only).
    gapPriority: "P0",
  },
  {
    id: "PU-PREVIEW-TXN",
    name: "Preview transactions (speculative, non-durable)",
    category: "generative",
    userValue: 9,
    pu: [
      { path: "src/index.ts", mustMatch: [/UpdateStreamParser/] },
    ],
    coresideImpl: [
      {
        path: "src-tauri/src/runtime_v2/preview_transaction.rs",
        mustMatch: [
          /preview_transaction_id/,
          /PreviewSurfaceModel/,
          /fn mark_committed|interrupted/,
        ],
      },
    ],
    coresideWired: [
      {
        path: "src-tauri/src/commands/message_cmds.rs",
        mustMatch: [/PreviewTransaction::new/, /mark_committed|interrupted/],
      },
    ],
    unitFiles: [
      {
        path: "src-tauri/src/runtime_v2/preview_transaction.rs",
        hints: [
          /preview_transaction_id/,
          /#\[test\]/,
          /cancel_after_speculative_paint_leaves_sqlite_unchanged|incomplete_progressive_finish_rolls_back/,
        ],
      },
    ],
  },
  {
    id: "PU-QUEUE",
    name: "Agent queue + queue UI",
    category: "coordination",
    userValue: 8,
    pu: [
      { path: "src/index.ts", mustMatch: [/withQueueMutation/] },
    ],
    coresideImpl: [
      {
        path: "src-tauri/src/runtime_v2/queue.rs",
        mustMatch: [/activate_next|recover_stale/],
      },
      {
        path: "src/components/chat/ConversationQueue.tsx",
        mustMatch: [/subscribeConversationQueue|ConversationQueue/],
      },
    ],
    coresideWired: [
      {
        path: "src/components/chat/ChatPanel.tsx",
        mustMatch: [/ConversationQueue/],
      },
    ],
    unitFiles: [
      {
        path: "src/components/chat/ConversationQueue.test.tsx",
        hints: [/ConversationQueue/, /expect\(/],
      },
      {
        path: "src/lib/tauri/queue-events.test.ts",
        hints: [/isQueueEventForConversation|expect\(/],
      },
    ],
    unitReports: ["reports/queue-product-results.json"],
  },
  {
    id: "PU-REPLAY",
    name: "History Replay (paced, read-only)",
    category: "history",
    userValue: 7,
    pu: [
      {
        path: "src/index.ts",
        anyMatch: [/replayHistory|parseReplay|replay/],
      },
    ],
    coresideImpl: [
      {
        path: "src/components/chat/ConversationHistory.tsx",
        mustMatch: [/function ReplayPlayer/, /listTransactions|list_turn_timeline|Replay/],
      },
    ],
    coresideWired: [
      {
        path: "src/components/chat/ChatPanel.tsx",
        mustMatch: [/ConversationHistory/],
      },
      {
        path: "src/components/chat/ConversationHistory.tsx",
        mustMatch: [/ReplayPlayer/, /applyOperations|undoTransaction/],
      },
    ],
    unitFiles: [
      {
        path: "src/components/chat/ConversationHistory.test.tsx",
        hints: [/ReplayPlayer/, /expect\(/],
      },
    ],
    unitReports: ["reports/replay-results.json"],
    // ReplayPlayer must NOT call apply — checked in classify via negative probe.
    rejectIfWiredMatches: [
      {
        path: "src/components/chat/ConversationHistory.tsx",
        // Soft: ReplayPlayer body should not invoke applyOperations.
        // Handled in notes, not hard fail.
      },
    ],
  },
  {
    id: "CS-BRANCH",
    name: "Conversation branches / forks",
    category: "history",
    userValue: 8,
    pu: [
      { path: "src/index.ts", mustMatch: [/registerFork|snapshotForFork/] },
    ],
    coresideImpl: [
      {
        path: "src-tauri/src/runtime_v2/branch.rs",
        mustMatch: [/chat_branches|create_branch|list_branches/],
      },
      {
        path: "src/components/chat/ConversationHistory.tsx",
        mustMatch: [/listBranches|branchConversation/],
      },
    ],
    coresideWired: [
      {
        path: "src/components/chat/ConversationHistory.tsx",
        mustMatch: [/branches/, /branchConversation/],
      },
      {
        path: "src/components/chat/ChatPanel.tsx",
        mustMatch: [/ConversationHistory/],
      },
    ],
    unitFiles: [
      {
        path: "src/components/chat/ConversationHistory.test.tsx",
        hints: [/listBranches|branch/],
      },
    ],
  },
  {
    id: "PU-FORMS",
    name: "Structured forms → model (StructuredUserInput)",
    category: "interaction",
    userValue: 9,
    pu: [
      {
        path: "src/initialPrompt.md",
        mustMatch: [/<form/, /clientSecret|hidden-submit-frame/],
      },
      {
        path: "src/index.ts",
        mustMatch: [/kind === \"form\"|route\.kind === \"form\"|formatFormPrompt/],
      },
    ],
    coresideImpl: [
      {
        path: "src-tauri/src/ai/structured_user_input.rs",
        mustMatch: [/StructuredUserInput/, /seal|trust/],
      },
    ],
    coresideWired: [
      {
        path: "src-tauri/src/commands/message_cmds.rs",
        mustMatch: [/StructuredUserInput/],
      },
      {
        path: "src/components/tool-canvas/ToolCanvas.tsx",
        anyMatch: [/submitToAgent|structured|appendContextLedger|form/],
      },
    ],
    unitFiles: [
      {
        path: "src-tauri/src/ai/structured_user_input.rs",
        hints: [/#\[test\]/, /spoofed_marker|seal_rejects|trust/],
      },
    ],
    unitReports: ["reports/structured-form-results.json"],
  },
  {
    id: "CS-CONTEXT-LEDGER",
    name: "Context ledger prompt injection",
    category: "interaction",
    userValue: 9,
    pu: [
      // PU uses form messages in history rather than a separate ledger table.
      { path: "src/index.ts", mustMatch: [/role.*form|addMessage\(\"form\"/] },
    ],
    coresideImpl: [
      {
        path: "src-tauri/src/runtime_v2/context_ledger.rs",
        mustMatch: [/ContextLedgerEntry/, /list_ledger_entries_for_inject|append_ledger/],
      },
    ],
    coresideWired: [
      {
        path: "src-tauri/src/commands/message_cmds.rs",
        mustMatch: [/list_ledger_entries_for_inject/, /model_context_only/],
      },
    ],
    unitFiles: [
      {
        path: "src-tauri/src/runtime_v2/context_ledger.rs",
        hints: [/#\[test\]/, /context_ledger/],
      },
    ],
  },
  {
    id: "CS-ACTION-LOG",
    name: "Action Log / Inspector",
    category: "observability",
    userValue: 6,
    pu: [
      { path: "src/debug.ts", anyMatch: [/debug|inspect/] },
    ],
    coresideImpl: [
      {
        path: "src/lib/action-log.ts",
        mustMatch: [/shouldShowActionLog|ActionLogMode/],
      },
      {
        path: "src/components/chat/ConversationHistory.tsx",
        mustMatch: [/inspector|Developer inspector/],
      },
    ],
    coresideWired: [
      {
        path: "src/components/chat/MessageList.tsx",
        mustMatch: [/shouldShowActionLog|showLiveActionLog/],
      },
      {
        path: "src/components/settings/AgentBehaviorSettings.tsx",
        mustMatch: [/ActionLogMode|action.?log/i],
      },
    ],
    unitFiles: [
      {
        path: "src/lib/action-log.test.ts",
        hints: [/shouldShowActionLog/, /expect\(/],
      },
    ],
  },
  {
    id: "PU-HTML-RESP",
    name: "Generated HTML bodies in protected webview",
    category: "security",
    userValue: 9,
    intentionallyRejected: true,
    rejectReason:
      "Arbitrary HTML in the protected webview is rejected; Coreside uses trusted declarative components.",
    pu: [
      {
        path: "src/initialPrompt.md",
        mustMatch: [/HTML bodies render|include JS, HTML, CSS/],
      },
    ],
    coresideRejectEvidence: [
      {
        path: "docs/GENERATIVE_UI_SECURITY_MODEL.md",
        anyMatch: [/never raw HTML|trusted declarative|no.*HTML/i],
      },
      {
        path: "src-tauri/src/ai/response_schema.rs",
        anyMatch: [/operation|schema|surface/],
      },
    ],
    // Dangerous if Coreside gained a raw HTML apply path:
    dangerousIfCoreside: [
      {
        path: "src/components/tool-renderer",
        // directory probe handled specially below
      },
    ],
  },
  {
    id: "PU-JS",
    name: "Arbitrary generated JavaScript",
    category: "security",
    userValue: 2,
    intentionallyRejected: true,
    rejectReason: "Model-generated script execution is rejected; registered actions only.",
    pu: [
      {
        path: "src/initialPrompt.md",
        mustMatch: [/<script>|JavaScript/],
      },
      {
        path: "src/index.ts",
        mustMatch: [/createElement\(\"script\"\)|Dynamic script/],
      },
    ],
    coresideRejectEvidence: [
      {
        path: "src-tauri/src/runtime_v2/packs.rs",
        anyMatch: [/pack|capability|allow/],
      },
      {
        path: "src/lib/actions.ts",
        anyMatch: [/register|action/],
      },
    ],
  },
  {
    id: "PU-CDN",
    name: "CDN Tailwind / D3 / CodeMirror injection",
    category: "security",
    userValue: 3,
    intentionallyRejected: true,
    rejectReason: "CDN script/link injection rejected; bundled capability packs only.",
    pu: [
      {
        path: "src/index.ts",
        mustMatch: [/unpkg\.com|cdn\.|script src=/],
      },
    ],
    coresideRejectEvidence: [
      {
        path: "src-tauri/src/runtime_v2/packs.rs",
        anyMatch: [/pack/],
      },
    ],
  },
  {
    id: "PU-FORM-ROUTE",
    name: "Hidden iframe form POST with clientSecret",
    category: "security",
    userValue: 8,
    intentionallyRejected: true,
    rejectReason:
      "iframe form POST + clientSecret trust path rejected; typed StructuredUserInput is the secure equivalent.",
    pu: [
      {
        path: "src/initialPrompt.md",
        mustMatch: [/hidden-submit-frame/, /clientSecret/],
      },
      {
        path: "src/index.ts",
        mustMatch: [/kind === \"form\"|route\.kind === \"form\"/],
      },
    ],
    coresideRejectEvidence: [
      {
        path: "src-tauri/src/ai/structured_user_input.rs",
        mustMatch: [/StructuredUserInput/, /trust/],
      },
    ],
  },
  {
    id: "PU-SCOPED-CSS",
    name: "Arbitrary generated CSS affecting protected UI",
    category: "security",
    userValue: 5,
    intentionallyRejected: true,
    rejectReason: "Global/model CSS on protected chrome rejected; semantic tokens + capability styles only.",
    pu: [
      {
        path: "src/initialPrompt.md",
        anyMatch: [/CSS|style=/],
      },
    ],
    coresideRejectEvidence: [
      {
        path: "docs/GENERATIVE_UI_SECURITY_MODEL.md",
        anyMatch: [/CSS|readability|token/i],
      },
    ],
  },
  {
    id: "PU-MULTIPLAYER",
    name: "Multiuser / Durable Object collaboration",
    category: "collaboration",
    userValue: 4,
    deferred: true,
    deferReason: "Public multiuser collaboration deferred; local-first consumer beta priority.",
    pu: [
      {
        path: "src/index.ts",
        anyMatch: [/DurableObject|WebSocket|multiplayer|audience/],
      },
    ],
  },
  {
    id: "PU-AUTH",
    name: "Hosted auth / Better Auth stack",
    category: "collaboration",
    userValue: 3,
    deferred: true,
    deferReason: "Hosted identity deferred relative to local BYOK consumer track.",
    pu: [
      { path: "src/auth/server.ts", anyMatch: [/betterAuth|auth/] },
    ],
  },
  {
    id: "CS-TXN",
    name: "Trusted application transactions",
    category: "runtime",
    userValue: 9,
    pu: [
      // PU applies DOM updates; Coreside equivalent is transactional ops.
      { path: "src/index.ts", mustMatch: [/UpdateStreamParser/] },
    ],
    coresideImpl: [
      {
        path: "src-tauri/src/application_kernel",
        // directory — special-cased
      },
      {
        path: "src-tauri/src/runtime_v2/transactions.rs",
        anyMatch: [/transaction|apply|Committed/],
      },
    ],
    coresideWired: [
      {
        path: "src-tauri/src/commands/message_cmds.rs",
        mustMatch: [/schedule_and_apply|apply_change/],
      },
    ],
    unitFiles: [
      {
        path: "reports/restore-transaction-results.json",
        // treated as report in classify via unitReports
        hints: [],
      },
    ],
    unitReports: ["reports/restore-transaction-results.json"],
  },
  {
    id: "CS-PACKS",
    name: "Capability packs (trusted components)",
    category: "runtime",
    userValue: 9,
    pu: [
      {
        path: "src/initialPrompt.md",
        mustMatch: [/HTML bodies render/],
      },
    ],
    coresideImpl: [
      {
        path: "src-tauri/src/runtime_v2/packs.rs",
        mustMatch: [/pack|Capability|register/],
      },
    ],
    coresideWired: [
      {
        path: "src-tauri/src/runtime_v2/mod.rs",
        mustMatch: [/packs/],
      },
    ],
  },
  {
    id: "PU-CHANNEL-SCOPE",
    name: "Scoped turn / Channel delivery (vs global broadcast)",
    category: "streaming",
    userValue: 8,
    pu: [
      { path: "src/index.ts", mustMatch: [/broadcast/] },
    ],
    coresideImpl: [
      {
        path: "src-tauri/src/commands/message_cmds.rs",
        mustMatch: [/Channel|on_event|PreviewSurface/],
      },
      {
        path: "src/lib/tauri/scoped-turn-streaming.test.ts",
        mustMatch: [/Channel|Text|expect\(/],
      },
    ],
    coresideWired: [
      {
        path: "src/stores/app-store.ts",
        mustMatch: [/on_event|Channel|PreviewSurface|Text/],
      },
    ],
    unitFiles: [
      {
        path: "src/lib/tauri/scoped-turn-streaming.test.ts",
        hints: [/expect\(/, /Channel|Text/],
      },
    ],
    unitReports: ["reports/scoped-streaming-results.json"],
  },
];

function dirHasMatchingFile(relDir, patterns) {
  const abs = path.join(root, relDir);
  if (!fs.existsSync(abs)) return false;
  const stack = [abs];
  while (stack.length) {
    const cur = stack.pop();
    for (const name of fs.readdirSync(cur)) {
      const p = path.join(cur, name);
      const st = fs.statSync(p);
      if (st.isDirectory()) {
        if (name === "node_modules" || name === "target") continue;
        stack.push(p);
      } else if (st.isFile() && /\.(rs|ts|tsx|md)$/.test(name)) {
        const text = readText(p);
        if (text && matchAny(text, patterns)) return true;
      }
    }
  }
  return false;
}

function probeDirOrFile(baseRoot, specs) {
  const normalized = [];
  for (const spec of specs || []) {
    const abs = path.join(baseRoot, spec.path);
    if (fs.existsSync(abs) && fs.statSync(abs).isDirectory()) {
      // Directory specs: anyMatch/mustMatch scanned shallowly.
      const patterns = spec.mustMatch || spec.anyMatch || [/.+/];
      const ok = (() => {
        const stack = [abs];
        let depth = 0;
        while (stack.length && depth < 200) {
          depth++;
          const cur = stack.pop();
          for (const name of fs.readdirSync(cur)) {
            const p = path.join(cur, name);
            const st = fs.statSync(p);
            if (st.isDirectory()) stack.push(p);
            else if (st.isFile()) {
              const text = readText(p);
              if (text && matchAny(text, patterns)) return true;
            }
          }
        }
        return false;
      })();
      normalized.push({
        path: spec.path,
        present: true,
        matched: ok,
      });
    } else {
      const one = probeFiles(baseRoot, [spec]);
      normalized.push(...one.hits, ...one.misses);
    }
  }
  const hits = normalized.filter((e) => e.matched);
  const misses = normalized.filter((e) => !e.matched);
  return {
    ok: hits.length > 0 && misses.length === 0,
    any: hits.length > 0,
    hits,
    misses,
  };
}

function classifyFeature(feature, ctx) {
  const notes = [];
  const evidence = [];

  const puProbe = probeFiles(ctx.puRoot, feature.pu);
  evidence.push({
    side: "partialUpdate",
    ok: puProbe.any,
    hits: puProbe.hits.map((h) => h.path),
    misses: puProbe.misses.map((m) => m.path),
  });

  if (feature.intentionallyRejected) {
    const rejectProbe = probeDirOrFile(root, feature.coresideRejectEvidence);
    evidence.push({
      side: "coresideReject",
      ok: rejectProbe.any,
      hits: rejectProbe.hits.map((h) => h.path),
    });
    // Guard: actual HTML injection (not deny-list string mentions).
    const dangerousHtml = dirHasMatchingFile("src/components/tool-renderer", [
      /dangerouslySetInnerHTML\s*=\s*\{/,
      /\beval\s*\(/,
      /new Function\s*\(/,
    ]);
    if (dangerousHtml && feature.id === "PU-HTML-RESP") {
      return {
        status: STATUS.Scaffolded,
        notes: [
          "Rejection intent documented, but tool-renderer shows dangerous HTML path — re-check.",
        ],
        evidence,
        partialUpdatePresent: puProbe.any,
      };
    }
    return {
      status: STATUS.IntentionallyRejected,
      notes: [feature.rejectReason, ...(rejectProbe.any ? [] : ["Reject evidence docs/code thin"])],
      evidence,
      partialUpdatePresent: puProbe.any,
    };
  }

  if (feature.deferred) {
    return {
      status: STATUS.Deferred,
      notes: [feature.deferReason],
      evidence,
      partialUpdatePresent: puProbe.any,
    };
  }

  const impl = probeDirOrFile(root, feature.coresideImpl);
  const wired = probeDirOrFile(root, feature.coresideWired);
  evidence.push({
    side: "coresideImpl",
    ok: impl.any,
    hits: impl.hits.map((h) => h.path),
    misses: impl.misses.map((m) => m.path),
  });
  evidence.push({
    side: "coresideWired",
    ok: wired.any,
    hits: wired.hits.map((h) => h.path),
    misses: wired.misses.map((m) => m.path),
  });

  if (!impl.any) {
    // Types-only / empty stubs?
    const stubish = (feature.coresideImpl || []).some((s) =>
      fs.existsSync(path.join(root, s.path)),
    );
    return {
      status: stubish ? STATUS.Scaffolded : STATUS.Absent,
      notes: [
        stubish
          ? "Path(s) exist but required content signals missing (not counted as implemented)."
          : "No Coreside implementation signals found.",
      ],
      evidence,
      partialUpdatePresent: puProbe.any,
    };
  }

  let unitOk = false;
  for (const uf of feature.unitFiles || []) {
    if (uf.path.endsWith(".json")) continue;
    if (hasSubstantiveTest(uf.path, uf.hints)) {
      unitOk = true;
      evidence.push({ side: "unitFile", path: uf.path, ok: true });
    } else {
      evidence.push({ side: "unitFile", path: uf.path, ok: false });
    }
  }
  for (const rel of feature.unitReports || []) {
    const { level, fresh, report } = reportEvidenceLevel(rel, {
      requireFreshFingerprint: false,
      fingerprint: ctx.sourceFingerprint,
    });
    const norm = normalizeEvidenceLevel(level);
    if (
      report &&
      (norm === STATUS.UnitVerified ||
        norm === STATUS.DesktopVerified ||
        report.result === "passed" ||
        report.result === "passed_unit" ||
        report.result === "Unit Verified")
    ) {
      // Accept unit report only if impl still probes true (avoid stale claim on deleted code).
      if (impl.any) {
        unitOk = true;
        evidence.push({
          side: "unitReport",
          path: rel,
          ok: true,
          fresh,
          level: norm || level,
        });
        if (!fresh && report.sourceFingerprint) {
          notes.push(`Unit report fingerprint drifted (${path.basename(rel)})`);
        }
      }
    }
  }

  let desktopOk = false;
  let packagedOk = false;

  // Fresh e2e-results journeys (current fingerprint) can elevate to Desktop Verified.
  const e2e = readJson("reports/e2e-results.json");
  const e2eFresh =
    e2e &&
    e2e.sourceFingerprint &&
    ctx.sourceFingerprint &&
    e2e.sourceFingerprint === ctx.sourceFingerprint &&
    Array.isArray(e2e.journeys) &&
    e2e.journeys.length > 0;
  if (e2eFresh && feature.e2eHints) {
    const hit = e2e.journeys.some(
      (j) =>
        (j.status === "passed" || j.result === "passed") &&
        matchAny(JSON.stringify(j), feature.e2eHints),
    );
    if (hit) {
      desktopOk = true;
      evidence.push({ side: "e2e-results", ok: true });
    }
  }

  for (const rel of feature.desktopReports || []) {
    const { level, fresh, report } = reportEvidenceLevel(rel, {
      requireFreshFingerprint: true,
      fingerprint: ctx.sourceFingerprint,
    });
    const norm = normalizeEvidenceLevel(level);
    const claimDesktop =
      report &&
      (norm === STATUS.DesktopVerified || report.evidenceLevel === "Desktop Verified");
    // Also require matching commit when present — drifted commits are historical only.
    const commitFresh =
      !report?.commit || !ctx.commit || report.commit === ctx.commit;
    if (claimDesktop && fresh && commitFresh) {
      desktopOk = true;
      evidence.push({ side: "desktopReport", path: rel, ok: true, fresh: true });
    } else if (claimDesktop) {
      notes.push(
        `Historical desktop evidence in ${path.basename(rel)} not applied (fingerprint/commit mismatch).`,
      );
      evidence.push({
        side: "desktopReport",
        path: rel,
        ok: false,
        fresh: false,
        commitFresh,
      });
    }
  }

  const packaged = reportEvidenceLevel("reports/packaged-smoke-results.json", {
    requireFreshFingerprint: true,
    fingerprint: ctx.sourceFingerprint,
  });
  if (
    packaged.report &&
    packaged.fresh &&
    (packaged.report.evidenceLevel === "Packaged Verified" ||
      packaged.report.result === "passed")
  ) {
    // Only elevate features that are part of packaged smoke when explicitly listed.
    if (feature.packagedRelevant) {
      packagedOk = true;
    }
  }

  // Replay safety note
  if (feature.id === "PU-REPLAY") {
    const hist = readText(path.join(root, "src/components/chat/ConversationHistory.tsx"));
    if (hist && /function ReplayPlayer[\s\S]{0,2500}applyOperations/.test(hist)) {
      notes.push("ReplayPlayer appears to reference applyOperations — verify read-only contract.");
    } else {
      notes.push("ReplayPlayer treated as read-only paced stepper (not PU stream re-execution).");
    }
  }

  if (feature.id === "PU-PROGRESSIVE-PREVIEW") {
    notes.push(
      "Secure progressive paint exists (PreviewTransaction + overlay + ToolCanvas badge); SQLite-backed cancel/incomplete rollback is unit-integrated; desktop/packaged progressive-surface journeys remain not_run (Journey 12 covers text streaming only).",
    );
  }

  let status;
  if (packagedOk) status = STATUS.PackagedVerified;
  else if (desktopOk) status = STATUS.DesktopVerified;
  else if (unitOk && wired.any) status = STATUS.UnitVerified;
  else if (unitOk && !wired.any) status = STATUS.UnitVerified; // backend unit without UI
  else if (wired.any && impl.ok) status = STATUS.IntegratedNotVerified;
  else if (wired.any) status = STATUS.IntegratedNotVerified;
  else if (impl.ok) status = STATUS.Implemented;
  else if (impl.any) status = STATUS.Scaffolded;
  else status = STATUS.Absent;

  // Cap: without wired UI/command path, do not claim Integrated.
  if (!wired.any && status === STATUS.IntegratedNotVerified) {
    status = STATUS.Implemented;
  }

  return {
    status,
    notes,
    evidence,
    partialUpdatePresent: puProbe.any,
    implOk: impl.ok || impl.any,
    wiredOk: wired.any,
    unitOk,
    desktopOk,
    packagedOk,
    gapPriority: feature.gapPriority || null,
  };
}

function loadSourceFingerprint() {
  const fpReport = readJson("reports/current-source-fingerprint.json");
  if (fpReport?.sourceFingerprint) return fpReport.sourceFingerprint;
  // Lightweight identity when baseline audit has not been run yet:
  // hash selected high-signal roots so Desktop claims still require freshness.
  const roots = [
    "package-lock.json",
    "src-tauri/Cargo.lock",
    "src-tauri/src/commands/message_cmds.rs",
    "src-tauri/src/runtime_v2/preview_transaction.rs",
    "src-tauri/src/ai/provider.rs",
  ];
  const hash = crypto.createHash("sha256");
  for (const rel of roots) {
    const abs = path.join(root, rel);
    if (!fs.existsSync(abs)) continue;
    hash.update(rel);
    hash.update("\0");
    hash.update(fs.readFileSync(abs));
    hash.update("\0");
  }
  return hash.digest("hex");
}

function main() {
  const commit = git(["rev-parse", "HEAD"]) || "unknown";
  const dirty = git(["status", "--porcelain"]).length > 0;
  const sourceFingerprint = loadSourceFingerprint();

  const archivePresent = fs.existsSync(PARTIAL_UPDATE_ARCHIVE);
  const observedSha = archivePresent ? sha256File(PARTIAL_UPDATE_ARCHIVE) : null;
  const archiveStatus = !archivePresent
    ? "archive_unavailable"
    : observedSha === EXPECTED_PARTIAL_UPDATE_SHA256
      ? "present_hash_match"
      : "present_hash_mismatch";

  if (archiveStatus === "present_hash_mismatch") {
    console.error(
      JSON.stringify(
        {
          command: commandName,
          error: "partial_update_archive_hash_mismatch",
          expected: EXPECTED_PARTIAL_UPDATE_SHA256,
          observed: observedSha,
          path: PARTIAL_UPDATE_ARCHIVE,
        },
        null,
        2,
      ),
    );
    process.exit(1);
  }

  const extracted = resolvePartialUpdateRoot(
    PARTIAL_UPDATE_ARCHIVE,
    EXPECTED_PARTIAL_UPDATE_SHA256,
  );

  if (!extracted.root) {
    console.error(
      JSON.stringify(
        {
          command: commandName,
          error: "partial_update_source_unavailable",
          archiveStatus,
          extract: extracted,
        },
        null,
        2,
      ),
    );
    process.exit(1);
  }

  const ctx = {
    puRoot: extracted.root,
    sourceFingerprint,
    commit,
  };

  const features = FEATURES.map((f) => {
    const result = classifyFeature(f, ctx);
    return {
      id: f.id,
      name: f.name,
      category: f.category,
      userValue: f.userValue ?? null,
      status: result.status,
      partialUpdatePresent: result.partialUpdatePresent,
      coreside: {
        implOk: result.implOk ?? null,
        wiredOk: result.wiredOk ?? null,
        unitOk: result.unitOk ?? null,
        desktopOk: result.desktopOk ?? null,
        packagedOk: result.packagedOk ?? null,
      },
      notes: result.notes,
      evidence: result.evidence,
      gapPriority: result.gapPriority,
      accessDate: ACCESS_DATE,
      rejectReason: f.rejectReason || null,
      deferReason: f.deferReason || null,
    };
  });

  const statusCounts = {};
  for (const s of Object.values(STATUS)) statusCounts[s] = 0;
  for (const f of features) {
    statusCounts[f.status] = (statusCounts[f.status] || 0) + 1;
  }

  // Highest-value remaining secure parity gap:
  // prefer P0 gaps that are Implemented/Unit Verified but not Desktop/Packaged.
  const gapCandidates = features
    .filter(
      (f) =>
        f.status !== STATUS.IntentionallyRejected &&
        f.status !== STATUS.Deferred &&
        f.status !== STATUS.Absent &&
        ![
          STATUS.DesktopVerified,
          STATUS.PackagedVerified,
          STATUS.CrossPlatformVerified,
          STATUS.HumanAccepted,
        ].includes(f.status),
    )
    .sort((a, b) => {
      const pr = (x) => (x.gapPriority === "P0" ? 0 : 1);
      if (pr(a) !== pr(b)) return pr(a) - pr(b);
      return (b.userValue || 0) - (a.userValue || 0);
    });

  const highestValueGap = gapCandidates[0]
    ? {
        id: gapCandidates[0].id,
        name: gapCandidates[0].name,
        status: gapCandidates[0].status,
        userValue: gapCandidates[0].userValue,
        rationale:
          gapCandidates[0].id === "PU-PROGRESSIVE-PREVIEW"
            ? "Progressive trusted surface paint is Unit Verified with SQLite cancel/incomplete rollback evidence (PreviewTransaction + overlay + ToolCanvas) but lacks desktop/packaged progressive-surface journeys — Journey 12 covers text streaming only."
            : gapCandidates[0].notes?.[0] ||
              "Highest user-value secure feature still below Desktop/Packaged Verified.",
      }
    : null;

  const matrix = {
    schemaVersion: 2,
    product: "Coreside",
    accessDate: ACCESS_DATE,
    generatedAt: nowIso(),
    commit,
    dirty,
    sourceFingerprint,
    command: commandName,
    exitStatus: 0,
    publicBeta: "Not ready",
    statusVocabulary: Object.values(STATUS),
    archive: {
      filename: path.basename(PARTIAL_UPDATE_ARCHIVE),
      path: PARTIAL_UPDATE_ARCHIVE,
      expectedSha256: EXPECTED_PARTIAL_UPDATE_SHA256,
      observedSha256: observedSha,
      status: archiveStatus,
    },
    partialUpdateSource: {
      root: path.relative(root, extracted.root).split(path.sep).join("/"),
      resolveSource: extracted.source,
      artifactRoot: extracted.artifactRoot
        ? path.relative(root, extracted.artifactRoot).split(path.sep).join("/")
        : null,
    },
    count: features.length,
    statusCounts,
    highestValueGap,
    methodology: [
      "Content probes on Partial Update and Coreside trees (mustMatch/anyMatch).",
      "File existence alone never upgrades status.",
      "Unit Verified requires substantive assert/expect tests or unit evidence reports with impl still present.",
      "Desktop/Packaged Verified require evidence reports matching current sourceFingerprint (or fresh e2e journeys).",
      "Historical desktop claims with drifted fingerprints are noted but not applied.",
      "HTML/JS/CSS/CDN/iframe form ports classified Intentionally Rejected when reject evidence holds.",
    ],
    features,
  };

  ensureDir(reportsDir);
  const matrixPath = path.join(reportsDir, "partial-update-feature-matrix.json");
  fs.writeFileSync(matrixPath, JSON.stringify(matrix, null, 2) + "\n");

  const mdPath = path.join(
    reportsDir,
    `partial-update-current-source-audit-${ACCESS_DATE}.md`,
  );
  const statusTable = Object.entries(statusCounts)
    .filter(([, n]) => n > 0)
    .map(([s, n]) => `| ${s} | ${n} |`)
    .join("\n");

  const featureRows = features
    .map(
      (f) =>
        `| \`${f.id}\` | ${f.name} | **${f.status}** | ${f.userValue ?? "—"} | ${(f.notes || []).slice(0, 1).join(" ") || "—"} |`,
    )
    .join("\n");

  const md = `# Partial Update Current-Source Audit

**Product:** Coreside  
**Access date:** ${ACCESS_DATE}  
**Generated:** ${matrix.generatedAt}  
**Commit:** \`${commit}\`  
**Dirty:** ${dirty ? "yes" : "no"}  
**Public beta:** **NOT READY**

## Archive

| Field | Value |
| --- | --- |
| Path | \`${PARTIAL_UPDATE_ARCHIVE}\` |
| Expected SHA-256 | \`${EXPECTED_PARTIAL_UPDATE_SHA256}\` |
| Observed SHA-256 | \`${observedSha || "n/a"}\` |
| Status | **${archiveStatus}** |
| Source root | \`${matrix.partialUpdateSource.root}\` (${extracted.source}) |

## Status counts

| Status | Count |
| --- | --- |
${statusTable}
| **Total** | **${features.length}** |

## Highest-value remaining secure parity gap

${
  highestValueGap
    ? `**${highestValueGap.id} — ${highestValueGap.name}** (${highestValueGap.status})

${highestValueGap.rationale}`
    : "_No open secure-parity gap identified._"
}

## Features

| ID | Feature | Status | Value | Note |
| --- | --- | --- | --- | --- |
${featureRows}

## Methodology

${matrix.methodology.map((m) => `- ${m}`).join("\n")}

## Commands

\`\`\`bash
npm run audit:partial-update
# optional:
PARTIAL_UPDATE_ARCHIVE="$HOME/Downloads/Partial Update Main.zip" npm run audit:partial-update
\`\`\`

Reports:

- \`reports/partial-update-feature-matrix.json\`
- \`reports/partial-update-current-source-audit-${ACCESS_DATE}.md\`
`;

  fs.writeFileSync(mdPath, md);

  console.log(
    JSON.stringify(
      {
        command: commandName,
        archiveStatus,
        partialUpdateSource: matrix.partialUpdateSource,
        count: features.length,
        statusCounts: Object.fromEntries(
          Object.entries(statusCounts).filter(([, n]) => n > 0),
        ),
        highestValueGap,
        matrixPath: path.relative(root, matrixPath),
        markdownPath: path.relative(root, mdPath),
      },
      null,
      2,
    ),
  );
}

main();
