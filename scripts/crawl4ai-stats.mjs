#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const isWin = process.platform === "win32";
const py = path.join(root, "services/crawl4ai/.venv", isWin ? "Scripts/python.exe" : "bin/python");
const script = path.join(root, "services/crawl4ai/scripts/doctor.py");
if (!fs.existsSync(py)) {
  console.error("missing venv");
  process.exit(1);
}
const args = process.argv.slice(2);
const mode = args[0] === "stats" || args.length === 0 ? ["stats"] : args;
const r = spawnSync(py, [script, ...mode], {
  stdio: "inherit",
  cwd: path.join(root, "services/crawl4ai"),
});
process.exit(r.status ?? 1);
