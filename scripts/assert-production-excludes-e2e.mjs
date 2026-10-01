#!/usr/bin/env node
/**
 * Fail if production Tauri config enables E2E/WebDriver surfaces.
 * Used by packaging CI (must stay shell-agnostic — no inline PowerShell quoting).
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cargo = fs.readFileSync(path.join(root, "src-tauri/Cargo.toml"), "utf8");
const conf = fs.readFileSync(path.join(root, "src-tauri/tauri.conf.json"), "utf8");

const defaultFeaturesMatch = cargo.match(
  /\[features\][\s\S]*?^default\s*=\s*\[([^\]]*)\]/m,
);
const defaultFeatures = defaultFeaturesMatch ? defaultFeaturesMatch[1] : "";
if (/\be2e\b/.test(defaultFeatures)) {
  console.error("e2e must not be in default features");
  process.exit(1);
}

if (conf.includes("wdio") || conf.includes("withGlobalTauri")) {
  console.error("production tauri.conf must not enable wdio / withGlobalTauri");
  process.exit(1);
}

console.log("production e2e exclusion OK");
