#!/usr/bin/env node
/**
 * Packaged smoke: scan bundle artifacts; on macOS, briefly launch a .app if present.
 *
 * Packaged Verified only when (1) the launched .app process matches the bundle
 * path and (2) package:scan exits 0. Launch-without-scan or any Coreside process
 * on the machine must not mint Packaged Verified. Missing artifact → not_run /
 * Scaffolded. Scan-only without launchable .app → Scaffolded.
 *
 * Writes: reports/packaged-smoke-results.json
 * Usage:  npm run test:packaged-smoke
 */
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const reportsDir = path.join(root, "reports");
const outPath = path.join(reportsDir, "packaged-smoke-results.json");

function git(args) {
  const r = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  return (r.stdout || "").trim();
}

function sha256File(filePath) {
  return crypto.createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
}

const cargoTarget = process.env.CARGO_TARGET_DIR?.trim();
const bundleRoots = [
  path.join(root, "src-tauri/target/release/bundle"),
  path.join(root, "src-tauri/target/debug/bundle"),
  ...(cargoTarget
    ? [
        path.join(cargoTarget, "release/bundle"),
        path.join(cargoTarget, "debug/bundle"),
      ]
    : []),
];

const ARTIFACT_RE = /\.(app|dmg|msi|msix|exe|AppImage|deb|rpm)$/i;

function walk(dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name.endsWith(".app")) out.push(full);
      else walk(full, out);
    } else if (ARTIFACT_RE.test(entry.name)) {
      out.push(full);
    }
  }
  return out;
}

function findMacApp(absoluteArtifacts) {
  return absoluteArtifacts.find((p) => p.endsWith(".app") && fs.existsSync(p)) ?? null;
}

/** PIDs whose command line includes this .app bundle path (not any Coreside). */
function pidsForBundledApp(appPath) {
  const pgrep = spawnSync("pgrep", ["-lf", "Coreside"], { encoding: "utf8" });
  if (pgrep.status !== 0) {
    return { pids: [], pgrepStatus: pgrep.status, pgrepStdout: (pgrep.stdout || "").slice(0, 500) };
  }
  const lines = (pgrep.stdout || "").split("\n").filter(Boolean);
  const pids = lines
    .filter((line) => line.includes(appPath))
    .map((line) => line.trim().split(/\s+/)[0])
    .filter((pid) => /^\d+$/.test(pid));
  return {
    pids,
    pgrepStatus: pgrep.status,
    pgrepStdout: (pgrep.stdout || "").slice(0, 500),
  };
}

/** SIGTERM only the PIDs that matched the launched bundle path. */
function quitBundledApp(pids) {
  let signaled = 0;
  for (const pid of pids) {
    const r = spawnSync("kill", ["-TERM", pid], { encoding: "utf8" });
    if (r.status === 0) signaled += 1;
  }
  return signaled > 0 ? 0 : 1;
}

/**
 * Launch macOS .app briefly, confirm *this* bundle is alive, then quit.
 * @returns {Promise<{ ok: boolean, detail: Record<string, unknown> }>}
 */
function launchMacApp(appPath) {
  const settleMs = Number(process.env.CORESIDE_PACKAGED_SMOKE_SETTLE_MS || 4000);
  return new Promise((resolve) => {
    const open = spawnSync("open", ["-n", appPath], { encoding: "utf8" });
    if (open.status !== 0) {
      resolve({
        ok: false,
        detail: {
          step: "open",
          status: open.status,
          stderr: (open.stderr || "").slice(0, 500),
        },
      });
      return;
    }

    setTimeout(() => {
      const alive = pidsForBundledApp(appPath);
      const running = alive.pids.length > 0;

      if (!running) {
        resolve({
          ok: false,
          detail: {
            step: "alive_check",
            pgrepStatus: alive.pgrepStatus,
            pgrepStdout: alive.pgrepStdout,
            settleMs,
            note: "No process command line contained the launched .app path (other Coreside instances do not count).",
          },
        });
        return;
      }

      const quitStatus = quitBundledApp(alive.pids);

      setTimeout(() => {
        resolve({
          ok: true,
          detail: {
            step: "launched_and_quit",
            settleMs,
            quitStatus,
            matchedPids: alive.pids,
            appPath: path.relative(root, appPath).replace(/\\/g, "/"),
          },
        });
      }, 800);
    }, settleMs);
  });
}

const artifactsAbs = [];
const artifacts = [];
const rootsPresent = [];
for (const br of bundleRoots) {
  if (!fs.existsSync(br)) continue;
  rootsPresent.push(path.relative(root, br).replace(/\\/g, "/"));
  for (const a of walk(br)) {
    artifactsAbs.push(a);
    artifacts.push(path.relative(root, a).replace(/\\/g, "/"));
  }
}

fs.mkdirSync(reportsDir, { recursive: true });
const commit = git(["rev-parse", "HEAD"]) || "unknown";
const dirty = git(["status", "--porcelain"]).length > 0;
const base = {
  schemaVersion: 1,
  product: "Coreside",
  generatedAt: new Date().toISOString(),
  commit,
  dirty,
  command: "test:packaged-smoke",
  launchedApp: false,
};

if (artifacts.length === 0) {
  const report = {
    ...base,
    status: "not_run",
    reason: "missing_artifact",
    evidenceLevel: "Scaffolded",
    bundleRootsChecked: bundleRoots.map((p) =>
      path.relative(root, p).replace(/\\/g, "/"),
    ),
    rootsPresent,
    artifacts: [],
    note: "No bundle artifact found — run `npm run build` first.",
    publicBeta: "Not ready",
  };
  fs.writeFileSync(outPath, JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify({ status: report.status, reason: report.reason, outPath }, null, 2));
  process.exit(0);
}

const scan = spawnSync("npm", ["run", "package:scan"], {
  cwd: root,
  encoding: "utf8",
  shell: true,
});
const packagingPath = path.join(reportsDir, "packaging-results.json");
let packaging = null;
if (fs.existsSync(packagingPath)) {
  try {
    packaging = JSON.parse(fs.readFileSync(packagingPath, "utf8"));
  } catch (e) {
    packaging = { parseError: String(e.message || e) };
  }
}

const artifactHashes = [];
for (const rel of artifacts) {
  const full = path.join(root, rel);
  try {
    const st = fs.statSync(full);
    if (st.isFile()) {
      artifactHashes.push({ path: rel, sha256: sha256File(full), size: st.size });
    } else {
      artifactHashes.push({
        path: rel,
        kind: "directory_bundle",
        note: "Directory bundle (.app) — no single-file hash.",
      });
    }
  } catch (e) {
    artifactHashes.push({ path: rel, error: String(e.message || e) });
  }
}

const macApp = process.platform === "darwin" ? findMacApp(artifactsAbs) : null;
const skipLaunch = process.env.CORESIDE_PACKAGED_SMOKE_NO_LAUNCH === "1";

const finish = (launch) => {
  const launchedOk = Boolean(launch?.ok);
  const scanOk = scan.status === 0;
  let status;
  let evidenceLevel;
  let reason;
  let note;

  if (launchedOk && scanOk) {
    status = "launch_passed";
    evidenceLevel = "Packaged Verified";
    reason = "artifact_scanned_and_launched";
    note =
      "Bundle scanned and macOS .app launched briefly then quit. Packaged Verified for smoke launch only — not a full product acceptance.";
  } else if (launchedOk && !scanOk) {
    // Launch alone must not mint Packaged Verified when package:scan failed.
    status = "launch_passed_scan_failed";
    evidenceLevel = "Scaffolded";
    reason = "launched_but_scan_failed";
    note =
      "App launched, but package:scan reported findings — evidenceLevel remains Scaffolded (not Packaged Verified). Review packaging-results.json.";
  } else if (macApp && !skipLaunch && launch && !launch.ok) {
    status = "launch_failed";
    evidenceLevel = "Scaffolded";
    reason = "launch_failed";
    note = "Bundle present but smoke launch failed.";
  } else {
    status = scanOk ? "scan_passed" : "scan_failed";
    evidenceLevel = "Scaffolded";
    reason = macApp
      ? skipLaunch
        ? "launch_skipped"
        : "artifact_present_scan_only"
      : "no_launchable_mac_app";
    note =
      "Bundle artifact found; package:scan executed. App was not launched successfully — evidenceLevel remains Scaffolded (not Packaged Verified).";
  }

  const report = {
    ...base,
    status,
    reason,
    evidenceLevel,
    launchedApp: launchedOk,
    launch: launch?.detail ?? null,
    rootsPresent,
    artifacts,
    artifactHashes,
    packageScanExitCode: scan.status ?? 1,
    packagingResults: packaging
      ? {
          status: packaging.status ?? null,
          rootsFound: packaging.rootsFound ?? null,
          filesScanned: packaging.filesScanned ?? null,
          findingsCount: Array.isArray(packaging.findings)
            ? packaging.findings.length
            : null,
        }
      : null,
    sourceIndependence: packaging?.sourceIndependence ?? null,
    note,
    publicBeta: "Not ready",
  };

  fs.writeFileSync(outPath, JSON.stringify(report, null, 2) + "\n");
  console.log(
    JSON.stringify(
      {
        status: report.status,
        evidenceLevel: report.evidenceLevel,
        launchedApp: report.launchedApp,
        artifacts: artifacts.length,
        outPath,
      },
      null,
      2,
    ),
  );

  if (
    status === "launch_failed" ||
    status === "scan_failed" ||
    status === "launch_passed_scan_failed"
  ) {
    process.exit(1);
  }
  process.exit(0);
};

if (macApp && !skipLaunch) {
  launchMacApp(macApp).then(finish);
} else {
  finish(null);
}
