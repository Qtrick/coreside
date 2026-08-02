#!/usr/bin/env node
/**
 * Scan packaged Tauri bundle outputs for forbidden artifacts.
 * Does not claim the package launched — only inspects files that exist.
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const cargoTarget = process.env.CARGO_TARGET_DIR?.trim();
const bundleRoots = [
  path.join(root, "src-tauri/target/release/bundle"),
  path.join(root, "src-tauri/target/debug/bundle"),
  ...(cargoTarget
    ? [
        path.join(cargoTarget, "release/bundle"),
        path.join(cargoTarget, "debug/bundle"),
      ]
    : []),
];

const FORBIDDEN_NAME_RE =
  /(^|\/)(\.env|\.mcp\.json|.*\.db|fixture.*\.sql|wdio.*conf|e2e_support)(\.|$)/i;
const FORBIDDEN_CONTENT_SNIPPETS = [
  "SUPABASE_ACCESS_TOKEN",
  "sk-ant-",
  "sk-proj-",
  "OPENAI_API_KEY=",
  "CORESIDE_E2E_SEED",
];

function walk(dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else out.push(full);
  }
  return out;
}

const findings = [];
let scanned = 0;
let rootsFound = 0;

for (const bundleRoot of bundleRoots) {
  if (!fs.existsSync(bundleRoot)) continue;
  rootsFound += 1;
  const files = walk(bundleRoot);
  for (const file of files) {
    scanned += 1;
    const rel = path.relative(root, file);
    if (FORBIDDEN_NAME_RE.test(rel.replace(/\\/g, "/"))) {
      findings.push({ severity: "P0", file: rel, reason: "forbidden_filename" });
      continue;
    }
    // Only text-sniff small-ish files
    let stat;
    try {
      stat = fs.statSync(file);
    } catch {
      continue;
    }
    if (stat.size > 2_000_000) continue;
    if (!/\.(json|plist|js|txt|html|toml|yml|yaml|env|md)$/i.test(file)) continue;
    let text = "";
    try {
      text = fs.readFileSync(file, "utf8");
    } catch {
      continue;
    }
    for (const snip of FORBIDDEN_CONTENT_SNIPPETS) {
      if (text.includes(snip)) {
        findings.push({
          severity: "P0",
          file: rel,
          reason: `forbidden_content:${snip}`,
        });
      }
    }
    if (text.includes("tauri-plugin-wdio") && !rel.includes("debug")) {
      findings.push({
        severity: "P1",
        file: rel,
        reason: "wdio_plugin_string_in_release_bundle_text",
      });
    }
  }
}

const outFile = path.join(root, "reports/packaging-results.json");
// Preserve manually recorded localMacosBuild evidence across rescans.
let priorLocalBuild;
try {
  if (fs.existsSync(outFile)) {
    const prior = JSON.parse(fs.readFileSync(outFile, "utf8"));
    if (prior && typeof prior === "object" && prior.localMacosBuild) {
      priorLocalBuild = prior.localMacosBuild;
    }
  }
} catch {
  // ignore corrupt prior
}

const result = {
  product: "Coreside",
  generatedAt: new Date().toISOString(),
  rootsFound,
  filesScanned: scanned,
  findings,
  status:
    rootsFound === 0
      ? "blocked"
      : findings.some((f) => f.severity === "P0")
        ? "failed"
        : findings.length
          ? "passed_with_warnings"
          : "passed",
  note:
    rootsFound === 0
      ? "No bundle directory found — run npm run build first"
      : undefined,
  ...(priorLocalBuild ? { localMacosBuild: priorLocalBuild } : {}),
};

fs.mkdirSync(path.join(root, "reports"), { recursive: true });
fs.writeFileSync(outFile, JSON.stringify(result, null, 2) + "\n");

console.log(JSON.stringify(result, null, 2));
if (result.status === "failed" || result.status === "blocked") process.exit(1);
