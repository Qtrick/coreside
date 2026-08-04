#!/usr/bin/env node
/**
 * Heuristic scan of consumer-visible risky terms under src/ (ts/tsx).
 * Approximate — string literals / JSX text only; does not execute React.
 *
 * Writes: reports/consumer-language-audit.json
 * Usage: npm run audit:consumer-language
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const srcRoot = path.join(root, "src");
const reportsDir = path.join(root, "reports");
const outPath = path.join(reportsDir, "consumer-language-audit.json");

/** @type {{ term: string; proposedCanonical: string; re: RegExp }[]} */
const RISKY = [
  {
    term: "Tool Canvas",
    proposedCanonical: "Apps panel / app beside chat",
    re: /Tool Canvas/g,
  },
  {
    term: "Added Settings",
    proposedCanonical: "App settings",
    re: /Added Settings/g,
  },
  {
    term: "Application Kernel",
    proposedCanonical: "(internal — avoid in consumer UI)",
    re: /Application Kernel/gi,
  },
  {
    term: "BYOK",
    proposedCanonical: "AI connections / your own key",
    re: /\bBYOK\b/g,
  },
  {
    term: "Manifest",
    proposedCanonical: "App details (avoid Manifest as UI label)",
    re: /\bManifest\b/g,
  },
  {
    term: "Gateway",
    proposedCanonical: "(internal — avoid in consumer UI)",
    re: /\bGateway\b/g,
  },
  {
    term: "Surface",
    proposedCanonical: "App / panel (avoid Surface as UI label)",
    re: /\bSurface\b/g,
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
    if (entry.isDirectory()) {
      if (entry.name === "node_modules" || entry.name === "dist") continue;
      walkTs(full, out);
    } else if (/\.(tsx|ts)$/.test(entry.name) && !entry.name.endsWith(".d.ts")) {
      out.push(full);
    }
  }
  return out;
}

/**
 * Heuristic: only flag matches inside string literals or JSX text nodes.
 * Skips import paths and type-only identifiers when possible.
 */
function collectFindings(rel, text) {
  const findings = [];
  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();
    if (trimmed.startsWith("//") || trimmed.startsWith("*") || trimmed.startsWith("/*")) {
      continue;
    }
    // Skip import/export path lines
    if (/^\s*(import|export)\b/.test(line) && /from\s+['"]/.test(line)) continue;

    for (const rule of RISKY) {
      rule.re.lastIndex = 0;
      let m;
      while ((m = rule.re.exec(line)) !== null) {
        const before = line.slice(0, m.index);
        const inString =
          /['"`]/.test(before) ||
          />[^<]*$/.test(before) ||
          /\{['"`]/.test(before);
        // JSX text: word appears outside of {} expression start
        const jsxText =
          /<\/?[A-Za-z]/.test(line) === false &&
          />/.test(before) &&
          !before.includes("{");
        // Broad: any quoted occurrence on the line
        const quoted =
          new RegExp(
            `['"\`][^'"\`]*${rule.term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`,
          ).test(line) ||
          line.includes(`"${rule.term}`) ||
          line.includes(`'${rule.term}`) ||
          line.includes(`\`${rule.term}`);

        if (!inString && !jsxText && !quoted) continue;

        findings.push({
          file: rel,
          line: i + 1,
          term: rule.term,
          proposedCanonical: rule.proposedCanonical,
          excerpt: line.trim().slice(0, 160),
        });
      }
    }
  }
  return findings;
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

const commit = git(["rev-parse", "HEAD"]) || null;
const dirty = git(["status", "--porcelain"]).length > 0;
const files = walkTs(srcRoot);
const findings = [];

for (const file of files) {
  const rel = path.relative(root, file).replace(/\\/g, "/");
  // Skip unit tests — they often assert internal labels.
  if (/\.test\.(ts|tsx)$/.test(rel)) continue;
  const text = fs.readFileSync(file, "utf8");
  findings.push(...collectFindings(rel, text));
}

const payload = {
  schemaVersion: 1,
  product: "Coreside",
  generatedAt: new Date().toISOString(),
  commit,
  dirty,
  fingerprint: readFingerprint(),
  command: "audit:consumer-language",
  scannedFiles: files.length,
  findingCount: findings.length,
  terms: RISKY.map((r) => ({
    term: r.term,
    proposedCanonical: r.proposedCanonical,
  })),
  findings,
};

fs.mkdirSync(reportsDir, { recursive: true });
fs.writeFileSync(outPath, JSON.stringify(payload, null, 2) + "\n");
console.log(
  `[audit:consumer-language] ${findings.length} finding(s) → ${path.relative(root, outPath)}`,
);
