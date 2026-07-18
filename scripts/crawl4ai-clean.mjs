#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const venv = path.join(root, "services/crawl4ai/.venv");
const data = path.join(root, "services/crawl4ai/.crawler-data");
if (fs.existsSync(venv)) {
  fs.rmSync(venv, { recursive: true, force: true });
  console.log("Removed .venv");
}
if (fs.existsSync(data)) {
  fs.rmSync(data, { recursive: true, force: true });
  console.log("Removed .crawler-data");
}
// Optional: clear disposable cache via cleanup if python still available elsewhere — skip.
console.log("crawl4ai:clean done");
