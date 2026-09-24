#!/usr/bin/env node
/**
 * Platform-aware `npm run dev` dispatcher.
 *
 * macOS  → packaged adaptive hot development (`tauri dev --runner …`)
 * others → plain `tauri dev`
 *
 * beforeDevCommand must remain `npm run dev:web` (never this script).
 */
import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const ROOT = resolve(fileURLToPath(new URL("..", import.meta.url)));
const RUNNER = join(ROOT, "scripts/macos-packaged-dev-runner.sh");
const require = createRequire(import.meta.url);

function resolveTauriCli() {
  try {
    return require.resolve("@tauri-apps/cli/tauri.js");
  } catch {
    return null;
  }
}

function run(command, args) {
  const child = spawn(command, args, {
    cwd: ROOT,
    stdio: "inherit",
    env: process.env,
  });
  const signals = ["SIGINT", "SIGTERM", "SIGHUP"];
  const forward = (sig) => {
    try {
      if (child.pid && !child.killed) process.kill(child.pid, sig);
    } catch {
      /* ignore */
    }
  };
  for (const sig of signals) {
    process.on(sig, forward);
  }
  const cleanup = () => {
    for (const sig of signals) {
      process.off(sig, forward);
    }
  };
  child.on("error", (err) => {
    cleanup();
    console.error(`dev: failed to spawn ${command}: ${err.message}`);
    process.exit(1);
  });
  child.on("exit", (code, signal) => {
    cleanup();
    if (signal) process.exit(1);
    process.exit(code ?? 1);
  });
}

function preflightMacos() {
  if (!existsSync(RUNNER)) {
    console.error(`dev: missing runner ${RUNNER}`);
    process.exit(1);
  }
  const verify = spawnSync(
    process.execPath,
    [join(ROOT, "scripts/verify-adaptive-icon.mjs")],
    { cwd: ROOT, stdio: "inherit" },
  );
  if (verify.status !== 0) {
    console.error(
      "dev: adaptive icon freshness failed — run npm run brand:compile-adaptive-icon",
    );
    process.exit(verify.status ?? 1);
  }
}

const tauriJs = resolveTauriCli();
if (!tauriJs) {
  console.error("dev: @tauri-apps/cli not found");
  process.exit(1);
}

const extraArgs = process.argv.slice(2);

if (process.platform === "darwin") {
  preflightMacos();
  process.env.CORESIDE_DEV_PREFLIGHT_DONE = "1";
  console.log("dev: macOS packaged adaptive hot development");
  console.log(`dev: runner=${RUNNER}`);
  run(process.execPath, [tauriJs, "dev", "--runner", RUNNER, ...extraArgs]);
} else {
  console.log(`dev: ${process.platform} uses raw tauri dev (no adaptive .app)`);
  run(process.execPath, [tauriJs, "dev", ...extraArgs]);
}
