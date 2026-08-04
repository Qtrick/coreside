#!/usr/bin/env node
/**
 * Run existing preservation unit tests and write honest evidence.
 *
 * Writes: reports/preservation-results.json
 * Usage:  npm run test:preservation-integration
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const reportsDir = path.join(root, "reports");
const outPath = path.join(reportsDir, "preservation-results.json");

function git(args) {
  const r = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  return (r.stdout || "").trim();
}

function run(cmd, args) {
  const r = spawnSync(cmd, args, { cwd: root, encoding: "utf8", shell: false });
  return {
    code: r.status ?? 1,
    stdout: (r.stdout || "").trim(),
    stderr: (r.stderr || "").trim(),
  };
}

const ts = run("npx", ["vitest", "run", "src/lib/preservation.test.ts"]);
const rust = run("cargo", [
  "test",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "--lib",
  "runtime_v2::preservation",
  "--",
  "--nocapture",
]);

const tsOk = ts.code === 0;
const rustOk = rust.code === 0;
const passed = tsOk && rustOk;

const report = {
  schemaVersion: 1,
  product: "Coreside",
  generatedAt: new Date().toISOString(),
  commit: git(["rev-parse", "HEAD"]) || "unknown",
  dirty: git(["status", "--porcelain"]).length > 0,
  command: "test:preservation-integration",
  status: passed ? "passed" : "failed",
  evidenceLevel: passed ? "Unit Verified" : "Scaffolded",
  note: passed
    ? "TypeScript + Rust preservation unit tests passed. Not Desktop/Packaged Verified."
    : "One or more preservation unit suites failed — see suiteResults.",
  suites: {
    typescript: {
      command: "vitest run src/lib/preservation.test.ts",
      exitCode: ts.code,
      passed: tsOk,
      source: "src/lib/preservation.ts",
      test: "src/lib/preservation.test.ts",
    },
    rust: {
      command:
        "cargo test --manifest-path src-tauri/Cargo.toml --lib runtime_v2::preservation",
      exitCode: rust.code,
      passed: rustOk,
      source: "src-tauri/src/runtime_v2/preservation.rs",
    },
  },
  publicBeta: "Not ready",
};

fs.mkdirSync(reportsDir, { recursive: true });
fs.writeFileSync(outPath, JSON.stringify(report, null, 2) + "\n");
console.log(
  JSON.stringify(
    {
      status: report.status,
      evidenceLevel: report.evidenceLevel,
      typescript: tsOk,
      rust: rustOk,
      outPath,
    },
    null,
    2,
  ),
);
if (ts.stdout) console.log(ts.stdout);
if (!tsOk && ts.stderr) console.error(ts.stderr);
if (rust.stdout) console.log(rust.stdout);
if (!rustOk && rust.stderr) console.error(rust.stderr);
process.exit(passed ? 0 : 1);
