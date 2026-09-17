#!/usr/bin/env node

/**
 * Developer Storage Audit & Safe Maintenance Tool
 *
 * Implements Section 49 & 50 of Coreside Storage Retention Policy:
 * - Differentiates development build artifacts from user runtime caches and protected data.
 * - Modes:
 *     --mode=audit    (Default: read-only inventory of disk usage by category)
 *     --mode=dry-run  (Simulates cleanup and reports space that would be freed)
 *     --mode=clean    (Executes safe cleanup using standard tools e.g. cargo clean)
 *
 * Safe-to-clean developer artifacts:
 * - Cargo target (`src-tauri/target`)
 * - Vite build cache (`.vite`, `dist`)
 * - Test coverage and reports (`coverage`, `playwright-report`, `e2e-results`, `logs/*.log`)
 *
 * Strictly Protected (Never automatically cleaned):
 * - Application source and git (`src`, `src-tauri/src`, `.git`)
 * - User databases (`*.db`, `*.sqlite`, `*.db-wal`)
 * - User credentials and settings
 * - Managed crawler service (`crawler/service`)
 */

import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = path.resolve(__dirname, "..");

// Parse arguments
const args = process.argv.slice(2);
let mode = "audit";
for (const arg of args) {
  if (arg.startsWith("--mode=")) {
    mode = arg.split("=")[1];
  } else if (arg === "--clean") {
    mode = "clean";
  } else if (arg === "--dry-run") {
    mode = "dry-run";
  }
}

function getDirSize(dirPath) {
  if (!fs.existsSync(dirPath)) return 0;
  let total = 0;
  try {
    const entries = fs.readdirSync(dirPath, { withFileTypes: true });
    for (const entry of entries) {
      const full = path.join(dirPath, entry.name);
      if (entry.isSymbolicLink()) continue;
      if (entry.isDirectory()) {
        total += getDirSize(full);
      } else if (entry.isFile()) {
        const stat = fs.statSync(full);
        total += stat.size;
      }
    }
  } catch {
    // Ignore unreadable entries
  }
  return total;
}

function formatBytes(bytes) {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(2)} ${units[i]}`;
}

const STORAGE_INVENTORY = [
  {
    category: "Rust Build Artifacts",
    path: path.join(ROOT_DIR, "src-tauri", "target"),
    safeToClean: true,
    cleanAction: () => {
      console.log("   -> Running cargo clean in src-tauri...");
      execSync("cargo clean", { cwd: path.join(ROOT_DIR, "src-tauri"), stdio: "inherit" });
    },
    reason: "Compiled Cargo dependencies and incremental debug/release objects",
  },
  {
    category: "Vite Cache & Build Outputs",
    path: path.join(ROOT_DIR, "node_modules", ".vite"),
    safeToClean: true,
    cleanAction: () => {
      fs.rmSync(path.join(ROOT_DIR, "node_modules", ".vite"), { recursive: true, force: true });
    },
    reason: "Pre-bundled Vite module cache",
  },
  {
    category: "Test Coverage & Reports",
    path: path.join(ROOT_DIR, "coverage"),
    safeToClean: true,
    cleanAction: () => {
      fs.rmSync(path.join(ROOT_DIR, "coverage"), { recursive: true, force: true });
    },
    reason: "Vitest test coverage reports",
  },
  {
    category: "E2E & Playwright Artifacts",
    path: path.join(ROOT_DIR, "e2e", ".artifacts"),
    safeToClean: true,
    cleanAction: () => {
      fs.rmSync(path.join(ROOT_DIR, "e2e", ".artifacts"), { recursive: true, force: true });
    },
    reason: "Playwright traces, videos, and screenshots",
  },
  {
    category: "Node Dependencies",
    path: path.join(ROOT_DIR, "node_modules"),
    safeToClean: false,
    reason: "Installed npm packages (use 'npm ci' or 'npm install' to manage)",
  },
  {
    category: "Crawler Managed Service",
    path: path.join(ROOT_DIR, "crawler", "service"),
    safeToClean: false,
    reason: "PROTECTED: Managed Crawl4AI Python virtual environment & service",
  },
  {
    category: "Git Repository Data",
    path: path.join(ROOT_DIR, ".git"),
    safeToClean: false,
    reason: "PROTECTED: Repository commit history and branches",
  },
];

console.log(`\n=== Coreside Storage Audit [Mode: ${mode.toUpperCase()}] ===\n`);

let totalSize = 0;
let cleanableSize = 0;

console.log(
  "Category".padEnd(28) +
  "Size".padEnd(14) +
  "Safe to Clean?".padEnd(16) +
  "Path"
);
console.log("-".repeat(90));

for (const item of STORAGE_INVENTORY) {
  const size = getDirSize(item.path);
  totalSize += size;
  if (item.safeToClean) {
    cleanableSize += size;
  }

  const exists = fs.existsSync(item.path);
  const sizeStr = exists ? formatBytes(size) : "[not present]";
  const safeStr = item.safeToClean ? "YES (dev-cache)" : "NO (protected)";

  console.log(
    item.category.padEnd(28) +
    sizeStr.padEnd(14) +
    safeStr.padEnd(16) +
    path.relative(ROOT_DIR, item.path)
  );
}

console.log("-".repeat(90));
console.log(`Total Inspected Footprint : ${formatBytes(totalSize)}`);
console.log(`Reclaimable Developer Cache: ${formatBytes(cleanableSize)}\n`);

if (mode === "dry-run") {
  console.log("[DRY RUN] The following items would be removed or cleaned:");
  for (const item of STORAGE_INVENTORY) {
    if (item.safeToClean && fs.existsSync(item.path)) {
      console.log(` - ${item.category} (${formatBytes(getDirSize(item.path))}): ${item.reason}`);
    }
  }
  console.log("\nTo execute cleanup, run: npm run clean:dev\n");
} else if (mode === "clean") {
  console.log("[CLEAN MODE] Executing safe developer cache cleanup...\n");
  for (const item of STORAGE_INVENTORY) {
    if (item.safeToClean && fs.existsSync(item.path) && item.cleanAction) {
      console.log(`Cleaning: ${item.category}...`);
      try {
        item.cleanAction();
        console.log(`  -> Cleaned successfully.`);
      } catch (err) {
        console.error(`  -> Failed to clean: ${err.message}`);
      }
    }
  }
  console.log("\nCleanup complete! Run 'npm run audit:storage' to verify reclaimed space.\n");
}
