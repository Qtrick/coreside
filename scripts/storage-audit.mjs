#!/usr/bin/env node

/**
 * Coreside developer storage detector / safe remover.
 *
 * Differentiates disposable build caches from protected source and user data.
 *
 * Modes:
 *   --mode=audit     read-only inventory (default)
 *   --mode=dry-run   list what would be removed
 *   --mode=clean     remove reclaimable caches now
 *   --mode=smart     clean only when reclaimable cache exceeds --threshold-gb
 *
 * Flags:
 *   --threshold-gb=N   smart trigger (default 4)
 *   --json             machine-readable summary
 *
 * Safe to clean:
 *   - src-tauri/target (via cargo clean, CARGO_TARGET_DIR unset)
 *   - Cursor sandbox cargo-target dirs
 *   - Vite cache, dist, coverage, e2e artifacts, logs/*.log
 *
 * Never cleaned:
 *   - source, migrations, permissions, configs
 *   - .git, node_modules, services/, databases, credentials
 */

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = path.resolve(__dirname, "..");
const TAURI_DIR = path.join(ROOT_DIR, "src-tauri");
const IN_REPO_TARGET = path.join(TAURI_DIR, "target");

const DEFAULT_THRESHOLD_GB = 4;

/** @typedef {"audit" | "dry-run" | "clean" | "smart"} Mode */
/** @typedef {{
 *   id: string,
 *   category: string,
 *   path: string,
 *   safeToClean: boolean,
 *   kind: "cargo-target" | "dir" | "logs" | "protected",
 *   reason: string,
 * }} InventoryItem */

export function parseArgs(argv = process.argv.slice(2)) {
  /** @type {Mode} */
  let mode = "audit";
  let thresholdGb = DEFAULT_THRESHOLD_GB;
  let json = false;

  for (const arg of argv) {
    if (arg.startsWith("--mode=")) {
      const m = arg.slice("--mode=".length);
      if (m === "audit" || m === "dry-run" || m === "clean" || m === "smart") {
        mode = m;
      } else {
        throw new Error(`Unknown mode: ${m}`);
      }
    } else if (arg === "--clean") mode = "clean";
    else if (arg === "--dry-run") mode = "dry-run";
    else if (arg === "--smart") mode = "smart";
    else if (arg === "--json") json = true;
    else if (arg.startsWith("--threshold-gb=")) {
      const n = Number(arg.slice("--threshold-gb=".length));
      if (!Number.isFinite(n) || n < 0) {
        throw new Error(`Invalid --threshold-gb: ${arg}`);
      }
      thresholdGb = n;
    } else if (arg === "--help" || arg === "-h") {
      mode = /** @type {Mode} */ ("help");
    }
  }
  return { mode, thresholdGb, json };
}

export function formatBytes(bytes) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(
    units.length - 1,
    Math.floor(Math.log(bytes) / Math.log(1024)),
  );
  return `${(bytes / Math.pow(1024, i)).toFixed(2)} ${units[i]}`;
}

/**
 * Disk usage in bytes. Prefer `du -sk` (closer to Finder / reclaimable space
 * on APFS) and fall back to a recursive walk.
 */
export function measurePathBytes(dirPath) {
  if (!fs.existsSync(dirPath)) return 0;
  try {
    const out = execFileSync("du", ["-sk", dirPath], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    const kb = Number.parseInt(String(out).trim().split(/\s+/)[0], 10);
    if (Number.isFinite(kb) && kb >= 0) return kb * 1024;
  } catch {
    // fall through
  }
  return walkBytes(dirPath);
}

function walkBytes(dirPath) {
  let total = 0;
  /** @type {string[]} */
  const stack = [dirPath];
  while (stack.length) {
    const cur = stack.pop();
    if (!cur) break;
    let entries;
    try {
      entries = fs.readdirSync(cur, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      const full = path.join(cur, entry.name);
      try {
        if (entry.isSymbolicLink()) continue;
        if (entry.isDirectory()) stack.push(full);
        else if (entry.isFile()) total += fs.statSync(full).size;
      } catch {
        // ignore
      }
    }
  }
  return total;
}

export function isCursorSandboxCargoTarget(absPath) {
  const norm = path.resolve(absPath);
  return (
    norm.includes(`${path.sep}cursor-sandbox-cache${path.sep}`) &&
    path.basename(norm) === "cargo-target"
  );
}

export function isProtectedPath(absPath, root = ROOT_DIR) {
  const rel = path.relative(root, path.resolve(absPath));
  if (!rel || rel.startsWith("..")) {
    // Outside repo: only sandbox cargo-targets are treatable as cleanable.
    return !isCursorSandboxCargoTarget(absPath);
  }
  const protectedPrefixes = [
    "src",
    "src-tauri/src",
    "src-tauri/migrations",
    "src-tauri/permissions",
    "src-tauri/icons",
    "src-tauri/resources",
    "src-tauri/gen",
    "services",
    "crawler",
    "supabase",
    ".git",
    ".cursor",
    ".codex",
    "docs",
    "public",
    "design",
  ];
  if (protectedPrefixes.some((p) => rel === p || rel.startsWith(`${p}/`))) {
    return true;
  }
  if (/\.(db|sqlite|db-wal|db-shm)$/i.test(rel)) return true;
  if (rel === "node_modules" || rel.startsWith("node_modules/")) {
    // node_modules itself is protected; only .vite under it is cleanable.
    return !rel.includes(`${path.sep}.vite`) && rel !== "node_modules/.vite";
  }
  return false;
}

/**
 * Discover reclaimable + protected inventory entries.
 * @param {{ root?: string, env?: NodeJS.ProcessEnv, tmpDir?: string }} [opts]
 * @returns {InventoryItem[]}
 */
export function discoverInventory(opts = {}) {
  const root = opts.root ?? ROOT_DIR;
  const env = opts.env ?? process.env;
  const tmpDir = opts.tmpDir ?? os.tmpdir();
  const tauri = path.join(root, "src-tauri");
  const inRepoTarget = path.join(tauri, "target");

  /** @type {InventoryItem[]} */
  const items = [
    {
      id: "rust-target",
      category: "Rust Build Artifacts",
      path: inRepoTarget,
      safeToClean: true,
      kind: "cargo-target",
      reason: "Compiled Cargo/Tauri debug+release objects (rebuilt on next build)",
    },
    {
      id: "vite-cache",
      category: "Vite Cache",
      path: path.join(root, "node_modules", ".vite"),
      safeToClean: true,
      kind: "dir",
      reason: "Pre-bundled Vite module cache",
    },
    {
      id: "dist",
      category: "Frontend Dist",
      path: path.join(root, "dist"),
      safeToClean: true,
      kind: "dir",
      reason: "Vite production build output (regenerated by build:web)",
    },
    {
      id: "coverage",
      category: "Test Coverage",
      path: path.join(root, "coverage"),
      safeToClean: true,
      kind: "dir",
      reason: "Vitest coverage reports",
    },
    {
      id: "e2e-artifacts",
      category: "E2E Artifacts",
      path: path.join(root, "e2e", ".artifacts"),
      safeToClean: true,
      kind: "dir",
      reason: "Webdriver/Playwright traces and screenshots",
    },
    {
      id: "logs",
      category: "Dev Logs",
      path: path.join(root, "logs"),
      safeToClean: true,
      kind: "logs",
      reason: "WDIO / Tauri service log files only (directory kept)",
    },
    {
      id: "node-modules",
      category: "Node Dependencies",
      path: path.join(root, "node_modules"),
      safeToClean: false,
      kind: "protected",
      reason: "PROTECTED: npm packages (use npm ci / npm install)",
    },
    {
      id: "services",
      category: "Managed Services",
      path: path.join(root, "services"),
      safeToClean: false,
      kind: "protected",
      reason: "PROTECTED: Crawl4AI / sidecar environments",
    },
    {
      id: "git",
      category: "Git Repository",
      path: path.join(root, ".git"),
      safeToClean: false,
      kind: "protected",
      reason: "PROTECTED: repository history",
    },
  ];

  // Active CARGO_TARGET_DIR (Cursor agents often redirect builds here).
  const cargoTargetDir = (env.CARGO_TARGET_DIR || "").trim();
  if (cargoTargetDir) {
    const resolved = path.resolve(cargoTargetDir);
    if (
      resolved !== path.resolve(inRepoTarget) &&
      !items.some((i) => path.resolve(i.path) === resolved)
    ) {
      const sandbox = isCursorSandboxCargoTarget(resolved);
      items.push({
        id: "cargo-target-dir-env",
        category: sandbox
          ? "Cursor Sandbox Cargo Target (active)"
          : "External CARGO_TARGET_DIR",
        path: resolved,
        safeToClean: sandbox,
        kind: sandbox ? "cargo-target" : "protected",
        reason: sandbox
          ? "Agent sandbox build cache (safe; not app source)"
          : "PROTECTED: custom CARGO_TARGET_DIR outside known sandbox layout",
      });
    }
  }

  // All Cursor sandbox cargo-target siblings under the system temp cache root.
  const sandboxRoot = path.join(tmpDir, "cursor-sandbox-cache");
  if (fs.existsSync(sandboxRoot)) {
    let children = [];
    try {
      children = fs.readdirSync(sandboxRoot, { withFileTypes: true });
    } catch {
      children = [];
    }
    for (const child of children) {
      if (!child.isDirectory()) continue;
      const candidate = path.join(sandboxRoot, child.name, "cargo-target");
      if (!fs.existsSync(candidate)) continue;
      const resolved = path.resolve(candidate);
      if (items.some((i) => path.resolve(i.path) === resolved)) continue;
      items.push({
        id: `sandbox-cargo-${child.name.slice(0, 12)}`,
        category: "Cursor Sandbox Cargo Target",
        path: resolved,
        safeToClean: true,
        kind: "cargo-target",
        reason: "Orphaned agent sandbox build cache",
      });
    }
  }

  // Defense: never mark protected prefixes as safe.
  for (const item of items) {
    if (item.safeToClean && isProtectedPath(item.path, root)) {
      item.safeToClean = false;
      item.kind = "protected";
      item.reason = `PROTECTED override: ${item.reason}`;
    }
  }

  return items;
}

export function coresideBuildProcessesRunning(root = ROOT_DIR) {
  const marker = path.join(root, "src-tauri", "target");
  try {
    const out = execFileSync("ps", ["-ax", "-o", "pid=,command="], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    return out
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean)
      .filter((line) => line.includes(marker))
      .map((line) => {
        const m = line.match(/^(\d+)\s+(.*)$/);
        return m ? { pid: Number(m[1]), command: m[2] } : null;
      })
      .filter(Boolean);
  } catch {
    return [];
  }
}

function rmDirSafe(absPath) {
  fs.rmSync(absPath, { recursive: true, force: true });
}

function cleanLogsDir(logsDir) {
  if (!fs.existsSync(logsDir)) return;
  for (const name of fs.readdirSync(logsDir)) {
    if (!name.endsWith(".log")) continue;
    fs.rmSync(path.join(logsDir, name), { force: true });
  }
}

/**
 * Clean one inventory item. Returns bytes roughly freed (pre-measure).
 * @param {InventoryItem} item
 * @param {{ root?: string }} [opts]
 */
export function cleanItem(item, opts = {}) {
  const root = opts.root ?? ROOT_DIR;
  const tauri = path.join(root, "src-tauri");
  const inRepoTarget = path.join(tauri, "target");

  if (!item.safeToClean) {
    throw new Error(`Refusing to clean protected item: ${item.id}`);
  }
  if (isProtectedPath(item.path, root) && item.kind !== "logs") {
    throw new Error(`Refusing protected path: ${item.path}`);
  }
  if (!fs.existsSync(item.path)) return 0;

  const before = measurePathBytes(item.path);

  if (item.kind === "logs") {
    cleanLogsDir(item.path);
    return before - measurePathBytes(item.path);
  }

  if (
    item.kind === "cargo-target" &&
    path.resolve(item.path) === path.resolve(inRepoTarget)
  ) {
    // Critical: do not inherit Cursor's redirected CARGO_TARGET_DIR.
    const env = { ...process.env };
    delete env.CARGO_TARGET_DIR;
    const result = spawnSync("cargo", ["clean"], {
      cwd: tauri,
      env,
      encoding: "utf8",
    });
    if (result.status !== 0) {
      // Fallback: direct remove if cargo clean fails (e.g. no toolchain briefly).
      rmDirSafe(item.path);
    }
    return before;
  }

  // Sandbox / other disposable cargo-target or dir.
  rmDirSafe(item.path);
  return before;
}

function printHelp() {
  console.log(`Usage: node scripts/storage-audit.mjs [options]

Options:
  --mode=audit|dry-run|clean|smart   Operation mode (default: audit)
  --smart                            Alias for --mode=smart
  --clean                            Alias for --mode=clean
  --dry-run                          Alias for --mode=dry-run
  --threshold-gb=N                   Smart clean threshold (default ${DEFAULT_THRESHOLD_GB})
  --json                             Print JSON summary
  -h, --help                         Show help

npm scripts:
  npm run audit:storage
  npm run clean:dev:dry
  npm run clean:dev
  npm run clean:dev:smart
`);
}

function main() {
  let parsed;
  try {
    parsed = parseArgs();
  } catch (err) {
    console.error(String(err?.message || err));
    process.exit(2);
  }

  if (parsed.mode === "help") {
    printHelp();
    process.exit(0);
  }

  const { mode, thresholdGb, json } = parsed;
  const inventory = discoverInventory();
  const projectBytes = measurePathBytes(ROOT_DIR);

  const rows = inventory.map((item) => {
    const exists = fs.existsSync(item.path);
    const bytes = exists ? measurePathBytes(item.path) : 0;
    return { ...item, exists, bytes };
  });

  const reclaimable = rows
    .filter((r) => r.safeToClean && r.exists)
    .reduce((sum, r) => sum + r.bytes, 0);
  const thresholdBytes = thresholdGb * 1024 * 1024 * 1024;
  const overThreshold = reclaimable >= thresholdBytes;
  const running = coresideBuildProcessesRunning();

  if (!json) {
    console.log(`\n=== Coreside Storage Detector [Mode: ${mode.toUpperCase()}] ===\n`);
    console.log(
      "Category".padEnd(36) +
        "Size".padEnd(14) +
        "Safe?".padEnd(8) +
        "Path",
    );
    console.log("-".repeat(100));
    for (const row of rows) {
      const sizeStr = row.exists ? formatBytes(row.bytes) : "[absent]";
      const safeStr = row.safeToClean ? "YES" : "NO";
      const displayPath = row.path.startsWith(ROOT_DIR)
        ? path.relative(ROOT_DIR, row.path)
        : row.path;
      console.log(
        row.category.padEnd(36) +
          sizeStr.padEnd(14) +
          safeStr.padEnd(8) +
          displayPath,
      );
    }
    console.log("-".repeat(100));
    console.log(`Project total (approx)     : ${formatBytes(projectBytes)}`);
    console.log(`Reclaimable developer cache: ${formatBytes(reclaimable)}`);
    console.log(
      `Smart threshold            : ${thresholdGb} GB (${overThreshold ? "TRIGGERED" : "ok"})`,
    );
    if (running.length) {
      console.log(
        `\nWarning: ${running.length} process(es) still using src-tauri/target — clean may fail until they exit.`,
      );
      for (const p of running.slice(0, 5)) {
        console.log(`  pid ${p.pid}: ${p.command.slice(0, 100)}`);
      }
    }
    console.log("");
  }

  /** @type {{ id: string, category: string, path: string, bytes: number, status: string, error?: string }[]} */
  const actions = [];

  const shouldClean =
    mode === "clean" || (mode === "smart" && overThreshold);

  if (mode === "dry-run" || (mode === "smart" && !overThreshold && !json)) {
    if (mode === "smart" && !overThreshold) {
      console.log(
        `[SMART] Reclaimable cache ${formatBytes(reclaimable)} is below ${thresholdGb} GB — no cleanup.`,
      );
      console.log("Force cleanup with: npm run clean:dev\n");
    } else if (mode === "dry-run") {
      console.log("[DRY RUN] Would remove:");
      for (const row of rows) {
        if (!row.safeToClean || !row.exists || row.bytes === 0) continue;
        console.log(
          ` - ${row.category} (${formatBytes(row.bytes)}): ${row.reason}`,
        );
      }
      console.log("\nExecute: npm run clean:dev   or   npm run clean:dev:smart\n");
    }
  }

  if (shouldClean) {
    if (mode === "smart" && !json) {
      console.log(
        `[SMART] Reclaimable cache ${formatBytes(reclaimable)} >= ${thresholdGb} GB — cleaning...\n`,
      );
    } else if (!json) {
      console.log("[CLEAN] Removing safe developer caches...\n");
    }

    if (running.length && !json) {
      console.log(
        "Note: build processes are running; in-repo cargo clean may partially fail and fall back to rm.\n",
      );
    }

    for (const row of rows) {
      if (!row.safeToClean || !row.exists) continue;
      if (row.bytes === 0 && row.kind !== "logs") continue;
      try {
        const freed = cleanItem(row);
        actions.push({
          id: row.id,
          category: row.category,
          path: row.path,
          bytes: freed,
          status: "cleaned",
        });
        if (!json) {
          console.log(
            `  cleaned ${row.category}: ~${formatBytes(freed)} (${row.id})`,
          );
        }
      } catch (err) {
        actions.push({
          id: row.id,
          category: row.category,
          path: row.path,
          bytes: 0,
          status: "failed",
          error: String(err?.message || err),
        });
        if (!json) {
          console.error(`  failed ${row.category}: ${err.message}`);
        }
      }
    }

    if (!json) {
      const freedSum = actions
        .filter((a) => a.status === "cleaned")
        .reduce((s, a) => s + a.bytes, 0);
      console.log(`\nCleanup finished. Approx reclaimed: ${formatBytes(freedSum)}`);
      console.log("Next Rust/Tauri build will fully recompile once.\n");
    }
  }

  if (json) {
    console.log(
      JSON.stringify(
        {
          mode,
          thresholdGb,
          overThreshold,
          projectBytes,
          reclaimableBytes: reclaimable,
          runningProcesses: running.length,
          inventory: rows.map((r) => ({
            id: r.id,
            category: r.category,
            path: r.path,
            safeToClean: r.safeToClean,
            kind: r.kind,
            exists: r.exists,
            bytes: r.bytes,
            reason: r.reason,
          })),
          actions,
        },
        null,
        2,
      ),
    );
  }

  // Non-zero exit when smart mode finds an oversized cache but was audit-only? No —
  // audit stays 0. Exit 1 only if a clean action failed.
  if (actions.some((a) => a.status === "failed")) process.exit(1);
}

const isMain =
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isMain) {
  main();
}
