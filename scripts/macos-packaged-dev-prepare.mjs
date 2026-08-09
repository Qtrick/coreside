#!/usr/bin/env node
/**
 * Build + install the macOS adaptive development .app for the packaged runner.
 * Invoked by macos-packaged-dev-runner.sh with cargo build flags (no app args).
 */
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import {
  DEBUG_BINARY,
  DEV_APP,
  DEV_EXECUTABLE,
  ROOT,
  TAURI_DIR,
  adHocSignDevApp,
  assertNotReleaseApp,
  conflictsForExpectedApp,
  ensureDevAppBundle,
  installDebugBinaryIntoDevApp,
  printIdentityReport,
  verifyDevBundleStructure,
} from "./lib/macos-dev-bundle.mjs";

function fail(msg, code = 1) {
  console.error(`macos-packaged-dev-prepare: ${msg}`);
  process.exit(code);
}

if (process.platform !== "darwin") {
  fail("macOS only");
}

assertNotReleaseApp(DEV_APP);

const cargoArgs = process.argv.slice(2).filter((a) => a !== "--");

const conflicts = conflictsForExpectedApp(DEV_APP);
if (conflicts.length > 0) {
  console.warn("macos-packaged-dev-prepare: other Coreside processes are running:");
  for (const p of conflicts) {
    console.warn(`  pid=${p.pid} exe=${p.executable}`);
  }
  console.warn(
    "Continuing with the exact development bundle. Prefer quitting the other instance.",
  );
  if (process.env.CORESIDE_DEV_FAIL_ON_CONFLICT === "1") {
    fail("conflicting Coreside process (CORESIDE_DEV_FAIL_ON_CONFLICT=1)");
  }
}

const build = spawnSync("cargo", ["build", ...cargoArgs], {
  cwd: TAURI_DIR,
  stdio: "inherit",
  env: process.env,
  shell: false,
});
if (build.error) fail(`cargo spawn failed: ${build.error.message}`);
if (build.status !== 0) process.exit(build.status ?? 1);
if (!existsSync(DEBUG_BINARY)) fail(`missing debug binary: ${DEBUG_BINARY}`);

const verify = spawnSync(
  process.execPath,
  [join(ROOT, "scripts/verify-adaptive-icon.mjs")],
  { cwd: ROOT, encoding: "utf8", shell: false },
);
if (verify.status !== 0) {
  process.stderr.write(verify.stdout || "");
  process.stderr.write(verify.stderr || "");
  fail("adaptive icon freshness check failed");
}

try {
  ensureDevAppBundle();
  installDebugBinaryIntoDevApp();
  adHocSignDevApp();
} catch (err) {
  fail(err instanceof Error ? err.message : String(err));
}

const structure = verifyDevBundleStructure(DEV_APP);
if (!structure.ok) {
  fail(`dev bundle incomplete: ${structure.errors.join("; ")}`);
}

printIdentityReport({
  appPath: DEV_APP,
  executablePath: DEV_EXECUTABLE,
  label: "adaptive development bundle",
});
