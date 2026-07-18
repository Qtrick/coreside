#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const isWin = process.platform === "win32";
const venv = path.join(root, "services/crawl4ai/.venv");
const bin = path.join(venv, isWin ? "Scripts/crawl4ai-doctor.exe" : "bin/crawl4ai-doctor");
if (!fs.existsSync(bin)) {
  console.error("Crawl4AI not installed. Run: npm run crawl4ai:setup");
  process.exit(1);
}
const r = spawnSync(bin, [], { stdio: "inherit" });
process.exit(r.status ?? 1);
