#!/usr/bin/env node
/**
 * Cross-platform adaptive icon freshness verifier.
 * Fails if Assets.car is missing, tiny, or stale vs Coreside.icon sources.
 * assetutil inspection is macOS-only (missing_optional elsewhere).
 *
 * Flags:
 *   --write-fingerprint  Write Assets.car.fingerprint from current sources
 *                        (used by brand:compile-adaptive-icon after actool).
 */
import { spawnSync } from "node:child_process";
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { resolve, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(fileURLToPath(new URL("..", import.meta.url)));
const ICON_SRC = join(ROOT, "src-tauri/icons/Coreside.icon");
const DEST_CAR = join(ROOT, "src-tauri/icons/Assets.car");
const FINGERPRINT_FILE = join(ROOT, "src-tauri/icons/Assets.car.fingerprint");
const PLIST = join(ROOT, "src-tauri/Info.plist");
const WRITE = process.argv.includes("--write-fingerprint");

function fail(msg) {
  console.error(`FAIL: ${msg}`);
  process.exit(1);
}

function sha256File(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function fingerprintSourcesSync() {
  const lines = ["actool-target:macosx-26.0", "app-icon:Icon"];
  const iconJson = join(ICON_SRC, "icon.json");
  if (!existsSync(iconJson)) fail("missing Coreside.icon/icon.json");
  lines.push(`${sha256File(iconJson)}  ${relative(ROOT, iconJson)}`);
  const assetsDir = join(ICON_SRC, "Assets");
  if (!existsSync(assetsDir)) fail("missing Coreside.icon/Assets");
  const files = readdirSync(assetsDir)
    .map((n) => join(assetsDir, n))
    .filter((p) => existsSync(p) && statSync(p).isFile())
    .sort((a, b) => a.localeCompare(b));
  for (const f of files) {
    lines.push(`${sha256File(f)}  ${relative(ROOT, f)}`);
  }
  return createHash("sha256").update(lines.join("\n") + "\n").digest("hex");
}

const expected = fingerprintSourcesSync();

if (WRITE) {
  writeFileSync(FINGERPRINT_FILE, `${expected}\n`, "utf8");
  console.log(`Wrote fingerprint ${expected}`);
  process.exit(0);
}

if (!existsSync(DEST_CAR)) {
  fail("Assets.car missing — run npm run brand:compile-adaptive-icon");
}
const size = statSync(DEST_CAR).size;
if (size < 10_000) {
  fail(`Assets.car appears corrupt/tiny (${size} bytes)`);
}
if (!existsSync(FINGERPRINT_FILE)) {
  fail("Assets.car.fingerprint missing — run npm run brand:compile-adaptive-icon");
}
const stored = readFileSync(FINGERPRINT_FILE, "utf8").trim();
if (stored !== expected) {
  console.error("FAIL: Assets.car is stale relative to Coreside.icon sources");
  console.error(`  stored:   ${stored}`);
  console.error(`  expected: ${expected}`);
  console.error("Re-run: npm run brand:compile-adaptive-icon");
  process.exit(1);
}

const plist = readFileSync(PLIST, "utf8");
if (!plist.includes("CFBundleIconName") || !plist.includes("<string>Icon</string>")) {
  fail("Info.plist missing CFBundleIconName=Icon");
}

if (process.platform === "darwin") {
  const probe = spawnSync("xcrun", ["--find", "assetutil"], { encoding: "utf8" });
  if (probe.status === 0) {
    const info = spawnSync(
      "xcrun",
      ["--sdk", "macosx", "assetutil", "--info", DEST_CAR],
      { encoding: "utf8", maxBuffer: 20 * 1024 * 1024 },
    );
    if (info.status !== 0) {
      fail(`assetutil could not read Assets.car: ${info.stderr || info.stdout}`);
    }
    const body = info.stdout || "";
    if (!/Icon/i.test(body)) {
      fail("Assets.car does not appear to contain Icon stack metadata");
    }
  } else {
    console.log("status=missing_optional reason=assetutil_unavailable");
  }
} else {
  console.log("status=not_applicable reason=non_macos_host assetutil_skipped");
}

console.log(`PASS adaptive icon freshness fingerprint=${expected} car_bytes=${size}`);
