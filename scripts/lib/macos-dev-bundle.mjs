/**
 * Shared helpers for the packaged macOS adaptive development bundle.
 * Paths are derived only from the repository root — never from user argv.
 */
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = resolve(fileURLToPath(new URL("../..", import.meta.url)));
export const TAURI_DIR = join(ROOT, "src-tauri");
export const DEV_APP = join(
  TAURI_DIR,
  "target/debug/bundle/macos/Coreside.app",
);
export const RELEASE_APP = join(
  TAURI_DIR,
  "target/release/bundle/macos/Coreside.app",
);
export const DEV_EXECUTABLE = join(DEV_APP, "Contents/MacOS/Coreside");
export const DEBUG_BINARY = join(TAURI_DIR, "target/debug/Coreside");
export const SOURCE_ASSETS_CAR = join(TAURI_DIR, "icons/Assets.car");
export const SOURCE_ICON_ICNS = join(TAURI_DIR, "icons/icon.icns");
export const SOURCE_INFO_PLIST = join(TAURI_DIR, "Info.plist");
export const SOURCE_BRANDING = join(TAURI_DIR, "resources/branding");
export const BUNDLE_ID = "com.coreside.app";

export function sha256File(filePath) {
  return createHash("sha256").update(readFileSync(filePath)).digest("hex");
}

/**
 * Exact directory mirror for repository-managed branding resources only.
 * Clears destination via temp + rename so deleted source files cannot linger.
 */
export function mirrorManagedDirectory(sourceDir, destDir) {
  assertUnderRoot(destDir, "mirror destination");
  if (!existsSync(sourceDir)) {
    throw new Error(`mirror source missing: ${sourceDir}`);
  }
  const parent = dirname(destDir);
  mkdirSync(parent, { recursive: true });
  const tmp = join(parent, `.mirror-tmp-${process.pid}-${Date.now()}`);
  assertUnderRoot(tmp, "mirror temp");
  try {
    rmSync(tmp, { recursive: true, force: true });
    cpSync(sourceDir, tmp, { recursive: true });
    rmSync(destDir, { recursive: true, force: true });
    renameSync(tmp, destDir);
  } catch (err) {
    // If dest was removed but rename failed, recover from tmp before cleanup.
    if (!existsSync(destDir) && existsSync(tmp)) {
      try {
        renameSync(tmp, destDir);
      } catch {
        /* fall through to original error */
      }
    }
    throw err;
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

/** Extract Coreside executable path from `ps` args — supports spaces in paths. */
export function executableFromPsArgs(args) {
  const text = String(args || "").trim();
  if (!text) return "";
  const appMatch = text.match(
    /^(.*?\.app\/Contents\/MacOS\/Coreside)(?:\s|$)/,
  );
  if (appMatch) return appMatch[1];
  // Bare Mach-O: only match actual binaries, not directory paths.
  // Must be /target/{debug,release}/Coreside (the compiled binary).
  const bareMatch = text.match(
    /^(.+\/target\/(?:debug|release)\/Coreside)(?:\s|$)/,
  );
  if (bareMatch) return bareMatch[1];
  return text.split(/\s+/)[0] || text;
}

export function assertUnderRoot(absPath, label = "path") {
  const resolved = resolve(absPath);
  const rootPrefix = ROOT.endsWith("/") ? ROOT : `${ROOT}/`;
  if (resolved !== ROOT && !resolved.startsWith(rootPrefix)) {
    throw new Error(`${label} escapes repository root: ${resolved}`);
  }
  return resolved;
}

export function assertNotReleaseApp(targetPath) {
  const resolved = resolve(targetPath);
  const releasePrefix = RELEASE_APP.endsWith("/")
    ? RELEASE_APP
    : `${RELEASE_APP}/`;
  if (resolved === RELEASE_APP || resolved.startsWith(releasePrefix)) {
    throw new Error(
      `refusing to mutate release bundle: ${resolved} (dev must use ${DEV_APP})`,
    );
  }
}

export function buildDevInfoPlist() {
  let plist = readFileSync(SOURCE_INFO_PLIST, "utf8");
  if (!plist.includes("CFBundleIconName")) {
    throw new Error("source Info.plist missing CFBundleIconName");
  }
  if (!plist.includes("<string>Icon</string>")) {
    throw new Error("source Info.plist missing CFBundleIconName=Icon");
  }
  if (!plist.includes("CFBundleIconFile")) {
    plist = plist.replace(
      /(<key>CFBundleIconName<\/key>\s*<string>Icon<\/string>)/,
      "$1\n  <key>CFBundleIconFile</key>\n  <string>icon.icns</string>",
    );
  }
  return plist;
}

export function ensureDevAppBundle({ forceResources = false } = {}) {
  assertNotReleaseApp(DEV_APP);
  assertUnderRoot(DEV_APP, "dev app");

  if (!existsSync(SOURCE_ASSETS_CAR)) {
    throw new Error(
      "missing src-tauri/icons/Assets.car — run brand:compile-adaptive-icon",
    );
  }
  if (statSync(SOURCE_ASSETS_CAR).size < 10_000) {
    throw new Error("Assets.car appears corrupt/tiny");
  }
  if (!existsSync(SOURCE_ICON_ICNS)) {
    throw new Error("missing src-tauri/icons/icon.icns");
  }
  if (!existsSync(SOURCE_BRANDING)) {
    throw new Error("missing src-tauri/resources/branding");
  }

  const contents = join(DEV_APP, "Contents");
  const macos = join(contents, "MacOS");
  const resources = join(contents, "Resources");
  const brandingDest = join(resources, "resources/branding");

  mkdirSync(macos, { recursive: true });
  mkdirSync(brandingDest, { recursive: true });

  writeFileSync(join(contents, "Info.plist"), buildDevInfoPlist(), "utf8");

  const carDest = join(resources, "Assets.car");
  const icnsDest = join(resources, "icon.icns");
  const needCar =
    forceResources ||
    !existsSync(carDest) ||
    sha256File(carDest) !== sha256File(SOURCE_ASSETS_CAR);
  const needIcns =
    forceResources ||
    !existsSync(icnsDest) ||
    sha256File(icnsDest) !== sha256File(SOURCE_ICON_ICNS);

  if (needCar) copyFileSync(SOURCE_ASSETS_CAR, carDest);
  if (needIcns) copyFileSync(SOURCE_ICON_ICNS, icnsDest);
  // Exact mirror — deleted source branding files must not linger in the debug .app.
  mirrorManagedDirectory(SOURCE_BRANDING, brandingDest);

  return {
    appPath: DEV_APP,
    executablePath: DEV_EXECUTABLE,
    assetsCarSha256: sha256File(carDest),
    sourceAssetsCarSha256: sha256File(SOURCE_ASSETS_CAR),
  };
}

export function installDebugBinaryIntoDevApp() {
  assertNotReleaseApp(DEV_APP);
  assertUnderRoot(DEV_EXECUTABLE, "dev executable");
  if (!existsSync(DEBUG_BINARY)) {
    throw new Error(`missing debug binary: ${DEBUG_BINARY}`);
  }
  mkdirSync(dirname(DEV_EXECUTABLE), { recursive: true });
  if (existsSync(DEV_EXECUTABLE)) {
    const srcStat = statSync(DEBUG_BINARY);
    const destStat = statSync(DEV_EXECUTABLE);
    if (srcStat.size === destStat.size && srcStat.mtimeMs <= destStat.mtimeMs) {
      return { path: DEV_EXECUTABLE, copied: false };
    }
  }
  const tmp = `${DEV_EXECUTABLE}.tmp-${process.pid}`;
  assertUnderRoot(tmp, "dev executable temp");
  try {
    copyFileSync(DEBUG_BINARY, tmp);
    chmodSync(tmp, 0o755);
    renameSync(tmp, DEV_EXECUTABLE);
    chmodSync(DEV_EXECUTABLE, 0o755);
  } finally {
    // After a successful rename, `tmp` is already gone; force keeps failures clean.
    rmSync(tmp, { force: true });
  }
  return { path: DEV_EXECUTABLE, copied: true };
}

export function adHocSignDevApp({ force = false } = {}) {
  assertNotReleaseApp(DEV_APP);
  if (process.platform !== "darwin") {
    return { status: "not_applicable" };
  }
  if (!force) {
    const verify = spawnSync("codesign", ["-v", DEV_APP], {
      encoding: "utf8",
      shell: false,
    });
    if (verify.status === 0) {
      return { status: "cached" };
    }
  }
  const r = spawnSync(
    "codesign",
    ["--force", "--deep", "-s", "-", DEV_APP],
    { encoding: "utf8", shell: false },
  );
  if (r.error?.code === "ENOENT") {
    throw new Error("codesign not found — Xcode CLT required for packaged dev");
  }
  if (r.status !== 0) {
    throw new Error(`codesign failed: ${r.stderr || r.stdout || r.status}`);
  }
  return { status: "pass" };
}

export function listCoresideProcesses() {
  if (process.platform !== "darwin") return [];
  const r = spawnSync("ps", ["-axo", "pid=,args="], { encoding: "utf8" });
  if (r.status !== 0 || !r.stdout) return [];
  const out = [];
  for (const line of r.stdout.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const m = /^(\d+)\s+(.*)$/.exec(trimmed);
    if (!m) continue;
    const pid = Number(m[1]);
    const args = m[2];
    // Full executable path — supports spaces; ignore editors whose args merely mention Coreside.
    const executable = executableFromPsArgs(args);
    if (!/\/Coreside$/.test(executable)) continue;
    if (/ps -axo|macos-packaged-dev-runner|scripts\/dev\.mjs/.test(args)) {
      continue;
    }
    let appPath = null;
    const appIdx = executable.indexOf(".app/");
    if (appIdx !== -1) {
      appPath = executable.slice(0, appIdx + 4);
    }
    out.push({ pid, args, executable, appPath });
  }
  return out;
}

/** Pure filter — unit-tested; `conflictsForExpectedApp` supplies live `ps` rows. */
export function selectConflictingProcesses(
  processes,
  expectedAppPath,
  { ignorePids = [] } = {},
) {
  const expected = resolve(expectedAppPath);
  const ignore = new Set(ignorePids.map(Number));
  return processes.filter((p) => {
    if (ignore.has(p.pid)) return false;
    if (!p.appPath) {
      return /\/Coreside$/.test(p.executable) || /Coreside$/.test(p.executable);
    }
    return resolve(p.appPath) !== expected;
  });
}

export function conflictsForExpectedApp(
  expectedAppPath,
  { ignorePids = [] } = {},
) {
  return selectConflictingProcesses(
    listCoresideProcesses(),
    expectedAppPath,
    { ignorePids },
  );
}

export function verifyDevBundleStructure(appPath = DEV_APP) {
  const errors = [];
  const contents = join(appPath, "Contents");
  const plistPath = join(contents, "Info.plist");
  const exe = join(contents, "MacOS/Coreside");
  const car = join(contents, "Resources/Assets.car");
  const icns = join(contents, "Resources/icon.icns");
  const branding = join(contents, "Resources/resources/branding");

  if (!existsSync(plistPath)) errors.push("missing Info.plist");
  if (!existsSync(exe)) errors.push("missing Contents/MacOS/Coreside");
  if (existsSync(exe)) {
    try {
      const mode = statSync(exe).mode;
      if ((mode & 0o111) === 0) errors.push("executable bit not set");
    } catch (e) {
      errors.push(`stat executable failed: ${e.message}`);
    }
  }
  if (!existsSync(car)) errors.push("missing Assets.car");
  if (!existsSync(icns)) errors.push("missing icon.icns");
  if (!existsSync(branding)) errors.push("missing branding resources");

  let plist = "";
  if (existsSync(plistPath)) {
    plist = readFileSync(plistPath, "utf8");
    if (
      !plist.includes("CFBundleIconName") ||
      !plist.includes("<string>Icon</string>")
    ) {
      errors.push("CFBundleIconName != Icon");
    }
    if (!plist.includes(BUNDLE_ID)) {
      errors.push(`CFBundleIdentifier missing ${BUNDLE_ID}`);
    }
  }

  let assetsMatch = false;
  let carSha = null;
  let sourceSha = null;
  if (existsSync(car) && existsSync(SOURCE_ASSETS_CAR)) {
    carSha = sha256File(car);
    sourceSha = sha256File(SOURCE_ASSETS_CAR);
    assetsMatch = carSha === sourceSha;
    if (!assetsMatch) errors.push("Assets.car SHA mismatch vs source catalog");
  }

  return {
    ok: errors.length === 0,
    errors,
    appPath,
    executablePath: exe,
    assetsCarSha256: carSha,
    sourceAssetsCarSha256: sourceSha,
    assetsMatch,
    plistHasIconName: plist.includes("CFBundleIconName"),
  };
}

export function printIdentityReport({
  appPath = DEV_APP,
  executablePath = DEV_EXECUTABLE,
  label = "adaptive development bundle",
} = {}) {
  const v = verifyDevBundleStructure(appPath);
  console.log(`Running ${label}: ${appPath}`);
  console.log(`Executable: ${executablePath}`);
  if (v.assetsCarSha256) {
    console.log(`Assets.car: ${v.assetsCarSha256}`);
  }
  console.log(`CFBundleIdentifier: ${BUNDLE_ID}`);
  console.log("CFBundleIconName: Icon");
  console.log(
    `Adaptive package requirements: ${
      v.ok ? "satisfied" : `FAILED (${v.errors.join("; ")})`
    }`,
  );
  return v;
}

/** Parse cargo-shaped argv from `tauri dev --runner`. */
export function splitCargoRunArgs(argv) {
  // argv[0] is script path when invoked via node; when exec'd as shebang, same.
  const args = [...argv];
  // Drop node + script if present.
  if (args[0] === process.execPath || /node$/.test(args[0] || "")) {
    args.shift();
  }
  if (args[0] && /macos-packaged-dev-runner\.mjs$/.test(args[0])) {
    args.shift();
  }

  const subcommand = args[0] || "";
  const rest = args.slice(1);
  const dd = rest.indexOf("--");
  const cargoSide = dd === -1 ? rest : rest.slice(0, dd);
  const appArgs = dd === -1 ? [] : rest.slice(dd + 1);
  return { subcommand, cargoArgs: cargoSide, appArgs };
}
