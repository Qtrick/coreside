#!/usr/bin/env node
/**
 * Inventory data-coreside-tour attributes vs tutorials.ts targetIds.
 *
 * Writes: reports/tutorial-target-inventory.json
 * Usage: npm run audit:tutorial-targets
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const srcRoot = path.join(root, "src");
const tutorialsPath = path.join(root, "src/lib/onboarding/tutorials.ts");
const reportsDir = path.join(root, "reports");
const outPath = path.join(reportsDir, "tutorial-target-inventory.json");

function git(args) {
  const r = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  return (r.stdout || "").trim();
}

function walkTs(dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "node_modules" || entry.name === "dist") continue;
      walkTs(full, out);
    } else if (/\.(tsx|ts)$/.test(entry.name) && !entry.name.endsWith(".d.ts")) {
      out.push(full);
    }
  }
  return out;
}

function readFingerprint() {
  const fpPath = path.join(reportsDir, "current-source-fingerprint.json");
  if (!fs.existsSync(fpPath)) return null;
  try {
    const j = JSON.parse(fs.readFileSync(fpPath, "utf8"));
    return j.sourceFingerprint ?? j.fingerprint ?? null;
  } catch {
    return null;
  }
}

/** Parse TOUR_TARGETS + step.target string literals from tutorials.ts */
function parseDeclaredTargets(text) {
  const targets = new Set();
  const tourBlock = text.match(/TOUR_TARGETS\s*=\s*\{([\s\S]*?)\}\s*as\s*const/);
  if (tourBlock) {
    for (const m of tourBlock[1].matchAll(/:\s*["']([^"']+)["']/g)) {
      targets.add(m[1]);
    }
  }
  for (const m of text.matchAll(/target:\s*TOUR_TARGETS\.(\w+)/g)) {
    // resolved via TOUR_TARGETS values already collected
    void m;
  }
  for (const m of text.matchAll(/target:\s*["']([^"']+)["']/g)) {
    targets.add(m[1]);
  }
  return [...targets].sort();
}

function collectDomTargets(files) {
  /** @type {{ id: string, file: string, line: number }[]} */
  const hits = [];
  const attrQuoted = /data-coreside-tour=["']([^"']+)["']/g;
  const attrBlock = /data-coreside-tour=\{([\s\S]*?)\}/g;

  const pushHit = (id, file, line) => {
    if (!id || id.includes("${")) return;
    if (hits.some((h) => h.id === id && h.file === file && h.line === line)) return;
    hits.push({ id, file, line });
  };

  for (const file of files) {
    const rel = path.relative(root, file).replace(/\\/g, "/");
    if (rel.includes("/onboarding/TutorialOverlay")) continue;
    const text = fs.readFileSync(file, "utf8");
    const lines = text.split(/\r?\n/);
    for (let i = 0; i < lines.length; i++) {
      attrQuoted.lastIndex = 0;
      let m;
      while ((m = attrQuoted.exec(lines[i])) !== null) {
        pushHit(m[1], rel, i + 1);
      }
    }
    attrBlock.lastIndex = 0;
    let bm;
    while ((bm = attrBlock.exec(text)) !== null) {
      const block = bm[1];
      const before = text.slice(0, bm.index);
      const line = before.split(/\r?\n/).length;
      // Prefer ternary consequent: ? "tour-id"
      const consequent = block.match(/\?\s*["']([^"']+)["']/);
      if (consequent) {
        pushHit(consequent[1], rel, line);
        continue;
      }
      const lit = block.match(/["']([^"']+)["']/);
      if (lit) pushHit(lit[1], rel, line);
    }
  }
  return hits;
}

const commit = git(["rev-parse", "HEAD"]) || null;
const dirty = git(["status", "--porcelain"]).length > 0;
const tutorialsText = fs.readFileSync(tutorialsPath, "utf8");
const declaredTargets = parseDeclaredTargets(tutorialsText);
const files = walkTs(srcRoot);
const domHits = collectDomTargets(files);
const presentIds = [...new Set(domHits.map((h) => h.id))].sort();

const missingInDom = declaredTargets.filter((id) => !presentIds.includes(id));
const undeclaredInTutorials = presentIds.filter(
  (id) => !declaredTargets.includes(id),
);

const payload = {
  schemaVersion: 1,
  product: "Coreside",
  generatedAt: new Date().toISOString(),
  commit,
  dirty,
  fingerprint: readFingerprint(),
  command: "audit:tutorial-targets",
  declaredTargets,
  presentInSource: presentIds,
  missingInDom,
  undeclaredInTutorials,
  occurrences: domHits,
};

fs.mkdirSync(reportsDir, { recursive: true });
fs.writeFileSync(outPath, JSON.stringify(payload, null, 2) + "\n");
console.log(
  `[audit:tutorial-targets] declared=${declaredTargets.length} present=${presentIds.length} missing=${missingInDom.length} → ${path.relative(root, outPath)}`,
);
