#!/usr/bin/env node
/**
 * Regenerate current-source baseline, fingerprint, and archive-vs-active comparison.
 *
 * Usage: npm run audit:current-source
 * Optional: CORESIDE_ARCHIVE=/path/to/archive.zip
 */
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const reportsDir = path.join(root, "reports");
const commandName = "audit:current-source";

const DEFAULT_ARCHIVE =
  process.env.CORESIDE_ARCHIVE ||
  path.join(process.env.HOME || "", "Downloads", "Coreside main.zip");
const EXPECTED_ARCHIVE_SHA256 =
  "ec8292249b58b565abef72baf285e36adce0ddea0059931166702d4ccf9bd228";
const PREVIOUS_ARCHIVE_SHA256 =
  "907a21f13ccbea5d7cbf7793bb5cb53098fa72836756b8f4c9a15569ef7c88c4";

const FINGERPRINT_ROOTS = [
  "src",
  "src-tauri/src",
  "src-tauri/capabilities",
  "src-tauri/permissions",
  "src-tauri/migrations",
  "src-tauri/prompts",
  "src-tauri/tauri.conf.json",
  "src-tauri/tauri.e2e.conf.json",
  "src-tauri/Cargo.toml",
  "src-tauri/Cargo.lock",
  "src-tauri/build.rs",
  "e2e",
  "scripts",
  "package.json",
  "package-lock.json",
  "vite.config.ts",
  "tsconfig.json",
  "eslint.config.js",
];

const EXCLUDE_DIR_NAMES = new Set([
  "node_modules",
  "target",
  "dist",
  ".git",
  ".reference",
  "gen",
]);

function run(cmd, args) {
  const r = spawnSync(cmd, args, { cwd: root, encoding: "utf8" });
  if (r.error) throw r.error;
  return { code: r.status ?? 1, stdout: (r.stdout || "").trim(), stderr: (r.stderr || "").trim() };
}

function sha256File(filePath) {
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(filePath));
  return hash.digest("hex");
}

function listFilesRecursive(absDir, relBase = "") {
  const out = [];
  if (!fs.existsSync(absDir)) return out;
  for (const name of fs.readdirSync(absDir)) {
    if (EXCLUDE_DIR_NAMES.has(name)) continue;
    if (name === ".artifacts") continue;
    const abs = path.join(absDir, name);
    const rel = path.join(relBase, name).split(path.sep).join("/");
    const st = fs.statSync(abs);
    if (st.isDirectory()) out.push(...listFilesRecursive(abs, rel));
    else if (st.isFile()) out.push(rel);
  }
  return out;
}

function collectFingerprintFiles() {
  const files = new Set();
  for (const entry of FINGERPRINT_ROOTS) {
    const abs = path.join(root, entry);
    if (!fs.existsSync(abs)) continue;
    const st = fs.statSync(abs);
    if (st.isFile()) {
      files.add(entry.split(path.sep).join("/"));
      continue;
    }
    for (const rel of listFilesRecursive(abs, entry)) files.add(rel);
  }
  return [...files].sort();
}

function fingerprintSource(files) {
  const hash = crypto.createHash("sha256");
  for (const rel of files) {
    const abs = path.join(root, rel);
    const bytes = fs.readFileSync(abs);
    hash.update(rel);
    hash.update("\0");
    hash.update(bytes);
    hash.update("\0");
  }
  return hash.digest("hex");
}

function countGlob(dir, predicate) {
  return listFilesRecursive(path.join(root, dir), dir).filter(predicate).length;
}

function archiveExtractRoot() {
  const preferred = path.join(root, ".reference", "coreside-rc3-archive", "coreside-main");
  if (fs.existsSync(preferred)) return preferred;
  return null;
}

function archiveExtractMatchesExpected() {
  const marker = path.join(root, ".reference", "coreside-rc3-archive", "source.sha256");
  if (!fs.existsSync(marker)) return false;
  return fs.readFileSync(marker, "utf8").trim() === EXPECTED_ARCHIVE_SHA256;
}

function compareTrees(activeRoot, archiveRoot) {
  const activeFiles = new Set(
    listFilesRecursive(activeRoot).filter(
      (f) =>
        !f.startsWith("reports/") &&
        !f.startsWith("target/") &&
        !f.includes("node_modules/") &&
        !f.endsWith(".DS_Store"),
    ),
  );
  // Compare selected roots only.
  const roots = ["src", "src-tauri/src", "src-tauri/migrations", "e2e", "scripts", "package.json", "package-lock.json", "src-tauri/Cargo.lock"];
  const onlyActive = [];
  const onlyArchive = [];
  const changed = [];
  for (const r of roots) {
    const aAbs = path.join(activeRoot, r);
    const bAbs = path.join(archiveRoot, r);
    const aFiles = fs.existsSync(aAbs)
      ? fs.statSync(aAbs).isFile()
        ? [r]
        : listFilesRecursive(aAbs, r)
      : [];
    const bFiles = fs.existsSync(bAbs)
      ? fs.statSync(bAbs).isFile()
        ? [r]
        : listFilesRecursive(bAbs, r)
      : [];
    const aSet = new Set(aFiles);
    const bSet = new Set(bFiles);
    for (const f of aSet) if (!bSet.has(f)) onlyActive.push(f);
    for (const f of bSet) if (!aSet.has(f)) onlyArchive.push(f);
    for (const f of aSet) {
      if (!bSet.has(f)) continue;
      const ah = sha256File(path.join(activeRoot, f));
      const bh = sha256File(path.join(archiveRoot, f));
      if (ah !== bh) changed.push(f);
    }
  }
  return {
    onlyInActive: onlyActive.sort(),
    onlyInArchive: onlyArchive.sort(),
    changed: changed.sort(),
    comparedRoots: roots,
  };
}

function nowIso() {
  return new Date().toISOString();
}

function writeJson(name, data) {
  fs.mkdirSync(reportsDir, { recursive: true });
  const p = path.join(reportsDir, name);
  fs.writeFileSync(p, JSON.stringify(data, null, 2) + "\n");
  return p;
}

function parsePorcelainPath(pathPart) {
  const raw = pathPart.includes(" -> ") ? pathPart.split(" -> ").pop() : pathPart;
  const trimmed = (raw || "").trim();
  if (trimmed.startsWith("\"") && trimmed.endsWith("\"") && trimmed.length >= 2) {
    return trimmed.slice(1, -1).replace(/\\"/g, "\"").replace(/\\\\/g, "\\");
  }
  return trimmed;
}

function parsePorcelain(stdout) {
  const staged = [];
  const modified = [];
  const untracked = [];
  for (const line of (stdout || "").split("\n")) {
    if (!line) continue;
    if (line.startsWith("! ")) continue; // ignored
    if (line.startsWith("??")) {
      untracked.push(parsePorcelainPath(line.slice(3)));
      continue;
    }
    // Porcelain v1: two status columns, one space, then path (or "old -> new").
    const m = /^(.)(.) (.*)$/.exec(line);
    if (!m) continue;
    const [, x, y, pathPart] = m;
    const filePath = parsePorcelainPath(pathPart);
    if (!filePath) continue;
    if (x !== " " && x !== "?") staged.push(filePath);
    if (y !== " " && y !== "?") modified.push(filePath);
  }
  return { staged, modified, untracked };
}

const commit = run("git", ["rev-parse", "HEAD"]).stdout;
const branch = run("git", ["rev-parse", "--abbrev-ref", "HEAD"]).stdout;
const porcelain = run("git", ["status", "--porcelain"]).stdout;
const dirty = porcelain.length > 0;
const { staged, modified, untracked } = parsePorcelain(porcelain);

const packageLockHash = sha256File(path.join(root, "package-lock.json"));
const cargoLockHash = sha256File(path.join(root, "src-tauri", "Cargo.lock"));
const fpFiles = collectFingerprintFiles();
const sourceFingerprint = fingerprintSource(fpFiles);

const node = run("node", ["-v"]).stdout;
const npm = run("npm", ["-v"]).stdout;
const rustc = run("rustc", ["--version"]).stdout;
const cargo = run("cargo", ["--version"]).stdout;
const tauriCli = run("npx", ["tauri", "--version"]).stdout;

const archivePath = DEFAULT_ARCHIVE;
const archivePresent = fs.existsSync(archivePath);
const observedSha = archivePresent ? sha256File(archivePath) : null;
const extractRoot = archiveExtractRoot();
const extractMatchesExpected = archiveExtractMatchesExpected();
const archiveStatus =
  !archivePresent
    ? extractRoot && extractMatchesExpected
      ? "zip_unavailable_extract_present"
      : extractRoot
        ? "zip_unavailable_extract_stale"
        : "archive_unavailable"
    : observedSha === EXPECTED_ARCHIVE_SHA256
      ? extractRoot && !extractMatchesExpected
        ? "present_hash_match_extract_stale"
        : "present_hash_match"
      : "present_hash_mismatch";

let archiveDiff = {
  status: "archive_unavailable",
  onlyInActive: [],
  onlyInArchive: [],
  changed: [],
};
if (
  (archiveStatus === "present_hash_match" ||
    archiveStatus === "zip_unavailable_extract_present") &&
  extractRoot
) {
  archiveDiff = {
    status:
      archiveStatus === "zip_unavailable_extract_present"
        ? "compared_against_prior_extract"
        : "compared",
    expectedArchiveSha256: EXPECTED_ARCHIVE_SHA256,
    note:
      archiveStatus === "zip_unavailable_extract_present"
        ? "Zip missing from Downloads; comparing against previously extracted .reference/coreside-rc3-archive/coreside-main (source.sha256 matches expected archive hash)."
        : undefined,
    ...compareTrees(root, extractRoot),
  };
} else if (
  archiveStatus === "present_hash_match_extract_stale" ||
  archiveStatus === "zip_unavailable_extract_stale"
) {
  archiveDiff = {
    status: "extract_stale_or_unmarked",
    hint: "Re-extract Coreside main.zip into .reference/coreside-rc3-archive and write source.sha256 with the expected archive hash",
    expectedArchiveSha256: EXPECTED_ARCHIVE_SHA256,
    onlyInActive: [],
    onlyInArchive: [],
    changed: [],
  };
} else if (archiveStatus === "present_hash_match") {
  archiveDiff = {
    status: "archive_present_but_extract_missing",
    hint: "Extract zip to .reference/coreside-rc3-archive/coreside-main then re-run",
    onlyInActive: [],
    onlyInArchive: [],
    changed: [],
  };
}

const migrations = fs.existsSync(path.join(root, "src-tauri", "migrations"))
  ? fs.readdirSync(path.join(root, "src-tauri", "migrations")).filter((f) => f.endsWith(".sql"))
      .length
  : 0;
const e2eSpecs = fs.existsSync(path.join(root, "e2e", "specs"))
  ? fs.readdirSync(path.join(root, "e2e", "specs")).filter((f) => f.endsWith(".ts")).length
  : 0;
const srcFiles = countGlob("src", () => true);
const rustFiles = countGlob("src-tauri/src", (f) => f.endsWith(".rs"));
const tauriCommands = (() => {
  const r = run("rg", ["-c", "#\\[tauri::command\\]", "src-tauri/src", "--glob", "*.rs"]);
  if (r.code !== 0 && !r.stdout) return null;
  return r.stdout
    .split("\n")
    .filter(Boolean)
    .reduce((sum, line) => sum + Number(line.split(":").pop() || 0), 0);
})();

const common = {
  schemaVersion: 1,
  product: "Coreside",
  generatedAt: nowIso(),
  commit,
  dirty,
  sourceFingerprint,
  packageLockHash,
  cargoLockHash,
  platform: process.platform,
  architecture: process.arch,
  buildMode: "development",
  command: commandName,
  exitStatus: 0,
};

const fingerprintReport = {
  ...common,
  algorithm: "sha256-concat-path-bytes",
  roots: FINGERPRINT_ROOTS,
  fileCount: fpFiles.length,
  inclusionRules: [
    "Tracked and untracked files under fingerprint roots",
    "Excludes node_modules, target, dist, .git, .reference, gen",
  ],
};

// Dirty trees are development evidence only — never claim public beta.
const publicBeta = "Not ready";
const hostedAi = "Not ready";

const baselineReport = {
  ...common,
  phase: "RC3",
  publicBeta,
  hostedAi,
  evidenceClass: dirty ? "development-dirty" : "development-clean",
  branch,
  dirtyState: dirty,
  stagedFiles: staged,
  modifiedFiles: modified,
  untrackedSourceFiles: untracked.filter(
    (f) =>
      f.startsWith("src/") ||
      f.startsWith("src-tauri/") ||
      f.startsWith("e2e/") ||
      f.startsWith("scripts/") ||
      f.startsWith("docs/"),
  ),
  archive: {
    filename: path.basename(archivePath),
    expectedPath: archivePath,
    expectedSha256: EXPECTED_ARCHIVE_SHA256,
    present: archivePresent,
    observedSha256: observedSha,
    status: archiveStatus,
    supersedes: PREVIOUS_ARCHIVE_SHA256,
  },
  counts: {
    srcFiles,
    rustFiles,
    migrations,
    e2eSpecs,
    tauriCommands,
    fingerprintFileCount: fpFiles.length,
  },
  toolVersions: {
    node,
    npm,
    rustc,
    cargo,
    tauriCli,
  },
  capabilities: fs.existsSync(path.join(root, "src-tauri", "capabilities"))
    ? fs.readdirSync(path.join(root, "src-tauri", "capabilities")).filter((f) => f.endsWith(".json"))
    : [],
  previousArchiveSuperseded: PREVIOUS_ARCHIVE_SHA256,
  notes: [
    `Archive status: ${archiveStatus}`,
    "Public beta remains Not ready until full current-source gates pass.",
    dirty
      ? "Dirty tree: development evidence only; cannot claim public beta."
      : "Clean tree at audit time; public beta still Not ready pending full gates.",
    "Open: packaged E2E/smoke, ACL raw-invoke denial proof, wallpaper pixel proof.",
  ],
};

const compareReport = {
  ...common,
  archive: baselineReport.archive,
  activeCommit: commit,
  activeDirty: dirty,
  comparison: archiveDiff.status,
  differences: {
    onlyInActiveCount: archiveDiff.onlyInActive?.length || 0,
    onlyInArchiveCount: archiveDiff.onlyInArchive?.length || 0,
    changedCount: archiveDiff.changed?.length || 0,
    onlyInActiveSample: (archiveDiff.onlyInActive || []).slice(0, 40),
    onlyInArchiveSample: (archiveDiff.onlyInArchive || []).slice(0, 40),
    changedSample: (archiveDiff.changed || []).slice(0, 80),
  },
  lockHashesMatchExternalNotes:
    packageLockHash ===
      "f32e3db1ed083c4c69903b536cb15edbb1ba616e8f8affd75c113cd7004e3be6" &&
    cargoLockHash ===
      "4b118413c6104abc1eab2df0455e2d2218df058c59c075866191c677709d0729",
  publicBeta,
  evidenceClass: dirty ? "development-dirty" : "development-clean",
};

writeJson("current-source-fingerprint.json", fingerprintReport);
writeJson("current-source-baseline.json", baselineReport);
writeJson("active-versus-uploaded-coreside.json", compareReport);

// Mark prior reports stale when fingerprint or archive expectation changes.
const freshnessEntries = fs
  .readdirSync(reportsDir)
  .filter((f) => f.endsWith(".json"))
  .sort()
  .map((name) => {
    const abs = path.join(reportsDir, name);
    let parsed = null;
    try {
      parsed = JSON.parse(fs.readFileSync(abs, "utf8"));
    } catch {
      parsed = null;
    }
    const reportFp = parsed && typeof parsed.sourceFingerprint === "string" ? parsed.sourceFingerprint : null;
    const reportCommit = parsed && typeof parsed.commit === "string" ? parsed.commit : null;
    const sameFp = reportFp === sourceFingerprint;
    const sameCommit = reportCommit === commit;
    let status = "unknown";
    if (name === "current-source-baseline.json" || name === "current-source-fingerprint.json" || name === "active-versus-uploaded-coreside.json" || name === "report-freshness-inventory.json") {
      status = "current";
    } else if (!parsed) {
      status = "unreadable";
    } else if (reportFp && !sameFp) {
      status = "stale_fingerprint";
    } else if (reportCommit && !sameCommit) {
      status = dirty ? "stale_or_dirty_tree" : "stale_commit";
    } else if (!reportFp && !reportCommit) {
      status = "missing_provenance";
    } else if (dirty) {
      status = "dirty_tree_untrusted";
    } else {
      status = "provisionally_current";
    }
    return {
      file: name,
      status,
      reportFingerprint: reportFp,
      reportCommit,
      mtimeMs: fs.statSync(abs).mtimeMs,
    };
  });

const freshnessReport = {
  ...common,
  command: "audit:current-source/report-freshness",
  currentFingerprint: sourceFingerprint,
  currentCommit: commit,
  archiveExpectedSha256: EXPECTED_ARCHIVE_SHA256,
  inventory: freshnessEntries,
  staleCount: freshnessEntries.filter((e) => String(e.status).startsWith("stale")).length,
  notes: [
    "Reports without matching sourceFingerprint are not release evidence.",
    "Dirty trees cannot claim public-beta readiness regardless of report freshness.",
  ],
};
writeJson("report-freshness-inventory.json", freshnessReport);

const md = `# Current Source Baseline (RC3)

**Product:** Coreside  
**Access date:** ${nowIso().slice(0, 10)}  
**Phase:** Public-beta release candidate 3  
**Public beta:** **NOT READY**  
**Hosted AI:** **NOT READY**

## Active repository

| Field | Value |
| --- | --- |
| Path | \`${root}\` |
| Branch | \`${branch}\` |
| Commit | \`${commit}\` |
| Dirty | ${dirty ? "Yes (development evidence only)" : "No"} |
| Source fingerprint | \`${sourceFingerprint}\` |

## Uploaded archive

| Field | Value |
| --- | --- |
| Filename | \`${path.basename(archivePath)}\` |
| Path | \`${archivePath}\` |
| Expected SHA-256 | \`${EXPECTED_ARCHIVE_SHA256}\` |
| Observed SHA-256 | \`${observedSha || "n/a"}\` |
| Status | **${archiveStatus}** |
| Supersedes | \`${PREVIOUS_ARCHIVE_SHA256}\` (prior expected / unavailable baseline) |

## Lockfiles

| File | SHA-256 |
| --- | --- |
| \`package-lock.json\` | \`${packageLockHash}\` |
| \`src-tauri/Cargo.lock\` | \`${cargoLockHash}\` |

## Tool versions

| Tool | Version |
| --- | --- |
| Node | ${node} |
| npm | ${npm} |
| rustc | ${rustc} |
| cargo | ${cargo} |
| Tauri CLI | ${tauriCli} |

## Counts

| Metric | Active |
| --- | --- |
| \`src\` files | ${srcFiles} |
| Rust \`.rs\` | ${rustFiles} |
| Migrations | ${migrations} |
| E2E specs | ${e2eSpecs} |
| Tauri commands | ${tauriCommands ?? "unknown"} |
| Fingerprint files | ${fpFiles.length} |

## Archive vs active

| Metric | Count |
| --- | --- |
| Comparison status | ${archiveDiff.status} |
| Only in active | ${archiveDiff.onlyInActive?.length || 0} |
| Only in archive | ${archiveDiff.onlyInArchive?.length || 0} |
| Changed | ${archiveDiff.changed?.length || 0} |

## Generation command

\`\`\`bash
npm run audit:current-source
\`\`\`

Reports:

- \`reports/current-source-baseline.json\`
- \`reports/current-source-fingerprint.json\`
- \`reports/active-versus-uploaded-coreside.json\`
- \`reports/report-freshness-inventory.json\`
`;

fs.writeFileSync(path.join(root, "docs", "CURRENT_SOURCE_BASELINE.md"), md);

console.log(
  JSON.stringify(
    {
      command: commandName,
      archiveStatus,
      sourceFingerprint,
      commit,
      dirty,
      fingerprintFileCount: fpFiles.length,
      archiveDiff: archiveDiff.status,
      onlyInActive: archiveDiff.onlyInActive?.length || 0,
      onlyInArchive: archiveDiff.onlyInArchive?.length || 0,
      changed: archiveDiff.changed?.length || 0,
      publicBeta: "Not ready",
    },
    null,
    2,
  ),
);

process.exit(
  archiveStatus === "present_hash_match" ||
    archiveStatus === "zip_unavailable_extract_present"
    ? 0
    : 1,
);
