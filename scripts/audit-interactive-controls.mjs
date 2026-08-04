#!/usr/bin/env node
/**
 * Static inventory of visible interactive controls under src/components.
 * Approximate — does not execute React or claim Desktop Verified.
 *
 * Writes:
 *   reports/interactive-control-inventory.json
 *   reports/dead-control-audit.json
 *
 * Usage:
 *   npm run audit:interactive-controls
 *   npm run audit:dead-controls
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const componentsRoot = path.join(root, "src/components");
const reportsDir = path.join(root, "reports");
const deadOnly = process.argv.includes("--dead-only");

const PATTERNS = [
  { id: "button", re: /<button\b/gi },
  { id: "role_button", re: /role=["']button["']/gi },
  { id: "type_submit", re: /type=["']submit["']/gi },
  { id: "onClick", re: /\bonClick\s*=/gi },
  { id: "anchor", re: /<a\s/gi },
  { id: "input_type", re: /<input\b[^>]*\btype\s*=/gi },
  { id: "select", re: /<select\b/gi },
  { id: "menuitem", re: /(?:role=["']menuitem["']|menuitem)/gi },
];

const FLAG_RE = {
  alert: /\balert\s*\(/i,
  TODO: /\bTODO\b/,
  placeholder: /\bplaceholder\b/i,
  not_implemented: /not\s+implemented/i,
};

const DEAD_HINTS = [
  { id: "alert_call", re: /\balert\s*\(/i },
  { id: "coming_soon", re: /coming\s+soon/i },
  { id: "stub", re: /\bstub\b/i },
  {
    id: "console_log_only_handler",
    // onClick={() => console.log(...)} or onClick={() => { console.log(...) }}
    re: /onClick\s*=\s*\{\s*(?:\(\)\s*=>\s*(?:\{\s*)?console\.log\b|\(\)\s*=>\s*console\.log\b)/i,
  },
];

function git(args) {
  const r = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  return (r.stdout || "").trim();
}

function walkTs(dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walkTs(full, out);
    else if (/\.(tsx|ts)$/.test(entry.name) && !entry.name.endsWith(".d.ts")) {
      out.push(full);
    }
  }
  return out;
}

function countMatches(text, re) {
  const flags = re.flags.includes("g") ? re.flags : re.flags + "g";
  const g = new RegExp(re.source, flags);
  return (text.match(g) || []).length;
}

const files = walkTs(componentsRoot);
const inventory = [];
const dead = [];

for (const file of files) {
  const rel = path.relative(root, file).replace(/\\/g, "/");
  const text = fs.readFileSync(file, "utf8");
  const counts = {};
  let total = 0;
  for (const p of PATTERNS) {
    const n = countMatches(text, p.re);
    if (n > 0) counts[p.id] = n;
    total += n;
  }
  if (total === 0) continue;

  const flags = {};
  for (const [k, re] of Object.entries(FLAG_RE)) {
    if (re.test(text)) flags[k] = true;
  }

  inventory.push({
    file: rel,
    approximateControlCount: total,
    counts,
    flags,
  });

  for (const hint of DEAD_HINTS) {
    const g = new RegExp(hint.re.source, hint.re.flags.includes("g") ? hint.re.flags : hint.re.flags + "g");
    let m;
    while ((m = g.exec(text))) {
      const line = text.slice(0, m.index).split("\n").length;
      dead.push({
        file: rel,
        line,
        hint: hint.id,
        snippet: text.slice(m.index, m.index + Math.min(80, text.length - m.index)).replace(/\s+/g, " ").trim(),
      });
    }
  }
}

inventory.sort((a, b) => a.file.localeCompare(b.file));
dead.sort((a, b) => a.file.localeCompare(b.file) || a.line - b.line);

const commit = git(["rev-parse", "HEAD"]) || "unknown";
const dirty = git(["status", "--porcelain"]).length > 0;
fs.mkdirSync(reportsDir, { recursive: true });

const inventoryReport = {
  schemaVersion: 1,
  product: "Coreside",
  generatedAt: new Date().toISOString(),
  commit,
  dirty,
  command: "audit:interactive-controls",
  evidenceLevel: "Scaffolded",
  note: "Static regex inventory only — not runtime interactivity proof.",
  filesScanned: files.length,
  filesWithControls: inventory.length,
  approximateTotalControls: inventory.reduce((s, f) => s + f.approximateControlCount, 0),
  files: inventory,
};

const deadReport = {
  schemaVersion: 1,
  product: "Coreside",
  generatedAt: new Date().toISOString(),
  commit,
  dirty,
  command: "audit:dead-controls",
  evidenceLevel: "Scaffolded",
  note: "Heuristic dead-control hints only — false positives expected.",
  matchCount: dead.length,
  matches: dead,
};

const inventoryPath = path.join(reportsDir, "interactive-control-inventory.json");
const deadPath = path.join(reportsDir, "dead-control-audit.json");
fs.writeFileSync(inventoryPath, JSON.stringify(inventoryReport, null, 2) + "\n");
fs.writeFileSync(deadPath, JSON.stringify(deadReport, null, 2) + "\n");

if (deadOnly) {
  console.log(JSON.stringify({ outPath: deadPath, matchCount: dead.length }, null, 2));
} else {
  console.log(
    JSON.stringify(
      {
        inventoryPath,
        deadPath,
        filesWithControls: inventory.length,
        approximateTotalControls: inventoryReport.approximateTotalControls,
        deadHints: dead.length,
      },
      null,
      2,
    ),
  );
}
