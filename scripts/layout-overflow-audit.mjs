#!/usr/bin/env node
/**
 * Layout overflow audit for Coreside (jsdom / static HTML fixture mode).
 * Produces reports/layout-overflow-result.json.
 *
 * For full runtime audits, launch the desktop app and inject the browser
 * probe below via DevTools. This CLI records a structured baseline + static
 * CSS risk scan so CI has an evidence artifact.
 */

import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");
const outPath = resolve(root, "reports/layout-overflow-result.json");
const baselinePath = resolve(root, "reports/layout-overflow-baseline.json");

const cssPath = resolve(root, "src/styles/global.css");
const tokensPath = resolve(root, "src/styles/tokens.css");

const findings = [];
const sizes = [
  { w: 900, h: 600 },
  { w: 1024, h: 700 },
  { w: 1280, h: 800 },
  { w: 1440, h: 900 },
  { w: 1728, h: 1117 },
];

function scanCssRisks(css) {
  const risks = [];
  // Fixed minmax without 0 can force min-content overflow in nested grids.
  const re = /minmax\(\s*(\d+(?:\.\d+)?(?:px|rem|em))\s*,/g;
  let m;
  while ((m = re.exec(css))) {
    risks.push({
      kind: "minmax_min_content_risk",
      value: m[0],
      index: m.index,
      note: "Prefer minmax(0, 1fr) or minmax(min(100%, N), 1fr) for nested grids.",
    });
  }
  if (/background-clip:\s*text/.test(css)) {
    risks.push({
      kind: "gradient_text",
      note: "Protected text must use solid theme colors, not background-clip text.",
    });
  }
  if (!/overflow-x:\s*hidden/.test(css) && !/\.app-shell[\s\S]{0,200}overflow/.test(css)) {
    risks.push({
      kind: "missing_root_overflow_guard",
      note: "Root overflow-x guard recommended after overflow causes are fixed.",
    });
  }
  return risks;
}

const css = readFileSync(cssPath, "utf8") + readFileSync(tokensPath, "utf8");
const cssRisks = scanCssRisks(css);

for (const risk of cssRisks) {
  findings.push({
    severity: risk.kind === "gradient_text" ? "P0" : "P2",
    selector: "css-scan",
    component: "styles",
    ...risk,
    sizes,
  });
}

// Contract checklist — documented expected owners (not live DOM).
const scrollOwners = [
  "sidebar",
  "message-list",
  "tool-canvas-body",
  "settings-panel / settings-content",
  "media-library",
  "projects-list",
];

const report = {
  generatedAt: new Date().toISOString(),
  mode: "static-css-scan",
  note: "Static scan + contract checklist. Full DOM probe requires desktop runtime.",
  viewportContract: "docs/APPLICATION_VIEWPORT_CONTRACT.md",
  sizes,
  scrollOwners,
  findingCount: findings.length,
  findings,
  cssRiskCount: cssRisks.length,
};

mkdirSync(resolve(root, "reports"), { recursive: true });
writeFileSync(outPath, JSON.stringify(report, null, 2) + "\n");

if (!existsSync(baselinePath)) {
  writeFileSync(
    baselinePath,
    JSON.stringify(
      {
        generatedAt: report.generatedAt,
        mode: "baseline-seed",
        findings: [],
      },
      null,
      2,
    ) + "\n",
  );
}

const p0 = findings.filter((f) => f.severity === "P0");
console.log(`layout-overflow-audit: ${findings.length} findings (${p0.length} P0)`);
console.log(`wrote ${outPath}`);
// Fail CI only on solid-text / contract-breaking CSS still present.
if (p0.length > 0) {
  console.error("P0 layout CSS risks remain — fix before release.");
  process.exitCode = 1;
}
