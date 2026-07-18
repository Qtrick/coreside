#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const isWin = process.platform === "win32";
const py = path.join(root, "services/crawl4ai/.venv", isWin ? "Scripts/python.exe" : "bin/python");
if (!fs.existsSync(py)) {
  console.error("Crawl4AI not installed. Run: npm run crawl4ai:setup");
  process.exit(1);
}
const r = spawnSync(py, ["-m", "pytest", "coreside_crawler/tests", "-q"], {
  stdio: "inherit",
  cwd: path.join(root, "services/crawl4ai"),
});
process.exit(r.status ?? 1);
