#!/usr/bin/env node
/**
 * Cross-platform Crawl4AI setup for Coreside.
 * Creates services/crawl4ai/.venv, installs crawl4ai==0.9.2, runs crawl4ai-setup.
 */
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const serviceDir = path.join(root, "services", "crawl4ai");
const venvDir = path.join(serviceDir, ".venv");
const isWin = process.platform === "win32";
const venvPython = path.join(venvDir, isWin ? "Scripts/python.exe" : "bin/python");
const PIN = "crawl4ai==0.9.2";

function run(cmd, args, opts = {}) {
  console.log(`> ${cmd} ${args.join(" ")}`);
  const r = spawnSync(cmd, args, { stdio: "inherit", cwd: serviceDir, ...opts });
  if (r.status !== 0) process.exit(r.status ?? 1);
}

function findPython() {
  const candidates = isWin
    ? ["py", "-3.13", "python", "python3"]
    : [
        "/Users/qunyingfan/miniconda3/bin/python3.13",
        "python3.13",
        "python3.12",
        "python3.11",
        "python3",
      ];
  // Prefer python3.13/3.12/3.11 via `which`-style tries
  const tries = isWin
    ? [
        ["py", ["-3.13", "-c", "import sys; print(sys.executable)"]],
        ["py", ["-3.12", "-c", "import sys; print(sys.executable)"]],
        ["python", ["-c", "import sys; print(sys.executable)"]],
      ]
    : [
        ["python3.13", ["-c", "import sys; print(sys.executable)"]],
        ["python3.12", ["-c", "import sys; print(sys.executable)"]],
        ["python3.11", ["-c", "import sys; print(sys.executable)"]],
        ["python3", ["-c", "import sys; print(sys.executable)"]],
      ];
  for (const [cmd, args] of tries) {
    const r = spawnSync(cmd, args, { encoding: "utf8" });
    if (r.status === 0 && r.stdout?.trim()) {
      const exe = r.stdout.trim();
      const ver = spawnSync(exe, ["-c", "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')"], {
        encoding: "utf8",
      });
      const v = ver.stdout?.trim() || "";
      const [maj, min] = v.split(".").map(Number);
      if (maj === 3 && min >= 10 && min < 14) {
        console.log(`Using Python ${v} at ${exe}`);
        return exe;
      }
      if (maj === 3 && min >= 10) {
        console.warn(`Warning: Python ${v} may be unsupported; prefer 3.11–3.13`);
        return exe;
      }
    }
  }
  console.error("No supported Python >=3.10 found. Install Python 3.13.");
  process.exit(1);
}

fs.mkdirSync(serviceDir, { recursive: true });
if (!fs.existsSync(venvPython)) {
  const py = findPython();
  run(py, ["-m", "venv", venvDir]);
}

run(venvPython, ["-m", "pip", "install", "--upgrade", "pip", "setuptools", "wheel"]);
run(venvPython, ["-m", "pip", "install", PIN]);
run(venvPython, ["-m", "pip", "install", "-e", "."]);
const freeze = spawnSync(venvPython, ["-m", "pip", "freeze"], { encoding: "utf8", cwd: serviceDir });
if (freeze.status === 0) {
  fs.writeFileSync(path.join(serviceDir, "requirements.lock"), freeze.stdout);
  console.log("Wrote requirements.lock");
}
const setupBin = path.join(venvDir, isWin ? "Scripts/crawl4ai-setup.exe" : "bin/crawl4ai-setup");
run(setupBin, []);
console.log("Crawl4AI setup complete.");
