/**
 * Unit tests for the packaged-dev dispatcher / command graph.
 * Run: node --test scripts/dev-dispatcher.test.mjs
 */
import assert from "node:assert/strict";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { describe, it } from "node:test";
import {
  ROOT,
  DEV_APP,
  RELEASE_APP,
  SOURCE_ASSETS_CAR,
  assertNotReleaseApp,
  assertUnderRoot,
  buildDevInfoPlist,
  splitCargoRunArgs,
  ensureDevAppBundle,
  verifyDevBundleStructure,
  installDebugBinaryIntoDevApp,
  selectConflictingProcesses,
  sha256File,
} from "./lib/macos-dev-bundle.mjs";

const packageJson = JSON.parse(
  readFileSync(join(ROOT, "package.json"), "utf8"),
);
const tauriConf = JSON.parse(
  readFileSync(join(ROOT, "src-tauri/tauri.conf.json"), "utf8"),
);

function readScript(rel) {
  return readFileSync(join(ROOT, rel), "utf8");
}

function fixtureApp({
  assetsBytes = "stale-assets-car-bytes",
  plist = `<?xml version="1.0"?><plist><dict>
  <key>CFBundleIdentifier</key><string>com.coreside.app</string>
  <key>CFBundleIconName</key><string>Icon</string>
</dict></plist>`,
  withExe = true,
} = {}) {
  const root = mkdtempSync(join(tmpdir(), "coreside-dev-bundle-"));
  const app = join(root, "Coreside.app");
  const contents = join(app, "Contents");
  const resources = join(contents, "Resources");
  mkdirSync(join(contents, "MacOS"), { recursive: true });
  mkdirSync(join(resources, "resources/branding"), { recursive: true });
  writeFileSync(join(contents, "Info.plist"), plist, "utf8");
  writeFileSync(join(resources, "Assets.car"), assetsBytes);
  writeFileSync(join(resources, "icon.icns"), "icns");
  writeFileSync(join(resources, "resources/branding/.keep"), "");
  if (withExe) {
    const exe = join(contents, "MacOS/Coreside");
    writeFileSync(exe, "#!/bin/sh\nexit 0\n");
    chmodSync(exe, 0o755);
  }
  return { root, app };
}

describe("command graph", () => {
  it("dev script is the platform dispatcher, not tauri recursively", () => {
    assert.equal(packageJson.scripts.dev, "node scripts/dev.mjs");
    assert.equal(packageJson.scripts["dev:web"], "vite");
    assert.equal(packageJson.scripts["dev:raw"], "tauri dev");
    assert.ok(packageJson.scripts["dev:bundle-verify"]);
  });

  it("beforeDevCommand does not recurse into npm run dev", () => {
    assert.equal(tauriConf.build.beforeDevCommand, "npm run dev:web");
    assert.notEqual(tauriConf.build.beforeDevCommand, "npm run dev");
    const devMjs = readScript("scripts/dev.mjs");
    assert.ok(!/spawnSync\(\s*["']npm["'][\s\S]*["']run["'][\s\S]*["']dev["']/.test(devMjs));
    assert.ok(!devMjs.includes("npm run dev\n"));
  });

  it("prepare and runner never re-enter npm run dev", () => {
    const prepare = readScript("scripts/macos-packaged-dev-prepare.mjs");
    const runner = readScript("scripts/macos-packaged-dev-runner.sh");
    assert.ok(!prepare.includes("npm run dev"));
    assert.ok(!runner.includes("npm run dev"));
    assert.ok(!prepare.includes('spawnSync("npm"'));
  });

  it("macOS runner ends in exec and never killall", () => {
    const runner = join(ROOT, "scripts/macos-packaged-dev-runner.sh");
    assert.ok(existsSync(runner));
    const sh = readFileSync(runner, "utf8");
    assert.ok(sh.includes('exec "$EXE"'));
    assert.ok(!sh.includes("killall"));
    assert.ok(sh.includes("macos-packaged-dev-prepare.mjs"));
  });

  it("packaged launch scripts never invoke killall", () => {
    const packaged = readScript("scripts/macos-run-packaged.sh");
    const prepare = readScript("scripts/macos-packaged-dev-prepare.mjs");
    const lib = readScript("scripts/lib/macos-dev-bundle.mjs");
    // Comments may mention the ban; executable lines must not call killall.
    const packagedCode = packaged
      .split("\n")
      .filter((line) => !line.trimStart().startsWith("#"))
      .join("\n");
    assert.ok(!packagedCode.includes("killall"));
    assert.ok(!prepare.includes("killall"));
    assert.ok(!lib.includes("killall"));
  });

  it("packaged launch gates conflicts separately from --replace-running", () => {
    const packaged = readScript("scripts/macos-run-packaged.sh");
    assert.ok(packaged.includes('bin !~ /\\/Coreside$/'));
    assert.ok(packaged.includes("coreside_ps conflict"));
    assert.ok(packaged.includes("coreside_ps same"));
    assert.ok(
      packaged.includes("does not clear other bundles"),
      "--replace-running must not be advertised as a conflict bypass",
    );
  });
});

describe("platform dispatcher source", () => {
  it("darwin chooses packaged runner; others choose raw tauri", () => {
    const src = readScript("scripts/dev.mjs");
    assert.ok(src.includes('process.platform === "darwin"'));
    assert.ok(src.includes("--runner"));
    assert.ok(src.includes("macos-packaged-dev-runner.sh"));
    // Non-darwin branch must stay bare tauri (no adaptive runner).
    assert.match(
      src,
      /else\s*\{\s*console\.log\(`dev: \$\{process\.platform\} uses raw tauri dev/,
    );
    const elseBlock = src.slice(src.indexOf("} else {"));
    assert.ok(elseBlock.includes('[tauriJs, "dev", ...extraArgs]'));
    assert.ok(!elseBlock.includes("--runner"));
  });
});

describe("release vs debug identity", () => {
  it("dev and release app paths are distinct debug/release bundles", () => {
    assert.ok(DEV_APP.includes("/target/debug/bundle/macos/Coreside.app"));
    assert.ok(RELEASE_APP.includes("/target/release/bundle/macos/Coreside.app"));
    assert.notEqual(resolve(DEV_APP), resolve(RELEASE_APP));
  });

  it("assertNotReleaseApp allows the debug development app", () => {
    assert.doesNotThrow(() => assertNotReleaseApp(DEV_APP));
    assert.doesNotThrow(() =>
      assertNotReleaseApp(join(DEV_APP, "Contents/MacOS/Coreside")),
    );
  });

  it("runner hardcodes debug APP and refuses override env", () => {
    const sh = readScript("scripts/macos-packaged-dev-runner.sh");
    assert.ok(sh.includes("target/debug/bundle/macos/Coreside.app"));
    assert.ok(!sh.includes("target/release/bundle/macos/Coreside.app"));
    assert.ok(sh.includes("CORESIDE_DEV_APP_OVERRIDE"));
    assert.ok(sh.includes("is not supported"));
  });
});

describe("hot reload / process replacement", () => {
  it("run path prepares then execs app; non-run forwards to cargo", () => {
    const sh = readScript("scripts/macos-packaged-dev-runner.sh");
    assert.ok(sh.includes('if [[ "$SUBCOMMAND" != "run" ]]; then'));
    assert.ok(sh.includes('exec cargo "$SUBCOMMAND"'));
    assert.ok(sh.includes('node "$PREPARE"'));
    // exec replaces the runner PID — required for Tauri SharedChild relaunch.
    const execIdx = sh.lastIndexOf('exec "$EXE"');
    const prepareIdx = sh.indexOf('node "$PREPARE"');
    assert.ok(prepareIdx !== -1 && execIdx !== -1 && prepareIdx < execIdx);
  });
});

describe("path security", () => {
  it("assertUnderRoot rejects paths that escape the repository", () => {
    assert.throws(() => assertUnderRoot("/tmp/evil.app", "path"), /escapes/);
    assert.throws(
      () => assertUnderRoot(join(ROOT, "..", "outside"), "path"),
      /escapes/,
    );
    assert.equal(assertUnderRoot(DEV_APP, "dev app"), resolve(DEV_APP));
  });
});

describe("conflicting processes", () => {
  it("flags other .app and bare Mach-O paths, ignores expected app and pids", () => {
    const expected = DEV_APP;
    const rows = [
      {
        pid: 11,
        executable: join(expected, "Contents/MacOS/Coreside"),
        appPath: expected,
      },
      {
        pid: 22,
        executable: join(RELEASE_APP, "Contents/MacOS/Coreside"),
        appPath: RELEASE_APP,
      },
      {
        pid: 33,
        executable: "/tmp/target/debug/Coreside",
        appPath: null,
      },
      {
        pid: 44,
        executable: "/usr/bin/something-else",
        appPath: null,
      },
    ];
    const conflicts = selectConflictingProcesses(rows, expected, {
      ignorePids: [22],
    });
    assert.deepEqual(
      conflicts.map((p) => p.pid),
      [33],
    );
  });

  it("prepare can fail closed on conflict via env gate", () => {
    const prepare = readScript("scripts/macos-packaged-dev-prepare.mjs");
    assert.ok(prepare.includes("conflictsForExpectedApp"));
    assert.ok(prepare.includes("CORESIDE_DEV_FAIL_ON_CONFLICT"));
  });
});

describe("bundle helpers", () => {
  it("refuses release bundle mutation", () => {
    assert.throws(() => assertNotReleaseApp(RELEASE_APP), /release bundle/);
    assert.throws(
      () => assertNotReleaseApp(join(RELEASE_APP, "Contents")),
      /release bundle/,
    );
  });

  it("dev Info.plist keeps CFBundleIconName=Icon", () => {
    const plist = buildDevInfoPlist();
    assert.ok(plist.includes("CFBundleIconName"));
    assert.ok(plist.includes("<string>Icon</string>"));
    assert.ok(plist.includes("com.coreside.app"));
  });

  it("splitCargoRunArgs separates app args after --", () => {
    const parsed = splitCargoRunArgs([
      process.execPath,
      join(ROOT, "scripts/macos-packaged-dev-runner.mjs"),
      "run",
      "--color",
      "always",
      "--",
      "--foo",
    ]);
    assert.equal(parsed.subcommand, "run");
    assert.deepEqual(parsed.cargoArgs, ["--color", "always"]);
    assert.deepEqual(parsed.appArgs, ["--foo"]);
  });

  it("ensureDevAppBundle creates a structural adaptive .app", () => {
    ensureDevAppBundle({ forceResources: true });
    const debugBin = join(ROOT, "src-tauri/target/debug/Coreside");
    if (existsSync(debugBin)) {
      installDebugBinaryIntoDevApp();
    } else {
      const exe = join(DEV_APP, "Contents/MacOS/Coreside");
      mkdirSync(join(DEV_APP, "Contents/MacOS"), { recursive: true });
      writeFileSync(exe, "#!/bin/sh\nexit 0\n");
      chmodSync(exe, 0o755);
    }
    const v = verifyDevBundleStructure(DEV_APP);
    assert.equal(v.ok, true, v.errors.join("; "));
    assert.equal(v.assetsMatch, true);
    assert.ok(
      resolve(v.appPath).endsWith("target/debug/bundle/macos/Coreside.app"),
    );
  });

  it("ensureDevAppBundle refreshes stale Assets.car from source catalog", () => {
    assert.ok(existsSync(SOURCE_ASSETS_CAR), "source Assets.car required");
    ensureDevAppBundle({ forceResources: true });
    const carDest = join(DEV_APP, "Contents/Resources/Assets.car");
    writeFileSync(carDest, "stale-not-matching-source-catalog");
    assert.notEqual(sha256File(carDest), sha256File(SOURCE_ASSETS_CAR));
    const beforeStale = verifyDevBundleStructure(DEV_APP);
    assert.equal(beforeStale.assetsMatch, false);
    assert.ok(beforeStale.errors.some((e) => /SHA mismatch/.test(e)));

    const refreshed = ensureDevAppBundle();
    assert.equal(refreshed.assetsCarSha256, sha256File(SOURCE_ASSETS_CAR));
    const after = verifyDevBundleStructure(DEV_APP);
    assert.equal(after.assetsMatch, true);
    assert.equal(after.ok, true, after.errors.join("; "));
  });

  it("verifyDevBundleStructure rejects wrong Info.plist icon identity", () => {
    const { root, app } = fixtureApp({
      plist: `<?xml version="1.0"?><plist><dict>
  <key>CFBundleIdentifier</key><string>com.coreside.app</string>
  <key>CFBundleIconName</key><string>AppIcon</string>
</dict></plist>`,
    });
    try {
      const v = verifyDevBundleStructure(app);
      assert.equal(v.ok, false);
      assert.ok(v.errors.some((e) => /CFBundleIconName/.test(e)));
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("verifyDevBundleStructure rejects missing CFBundleIdentifier", () => {
    const { root, app } = fixtureApp({
      plist: `<?xml version="1.0"?><plist><dict>
  <key>CFBundleIconName</key><string>Icon</string>
</dict></plist>`,
    });
    try {
      const v = verifyDevBundleStructure(app);
      assert.equal(v.ok, false);
      assert.ok(v.errors.some((e) => /CFBundleIdentifier/.test(e)));
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("verifyDevBundleStructure rejects Assets.car SHA mismatch vs source", () => {
    const { root, app } = fixtureApp({
      assetsBytes: "definitely-not-the-source-catalog",
    });
    try {
      const v = verifyDevBundleStructure(app);
      assert.equal(v.ok, false);
      assert.equal(v.assetsMatch, false);
      assert.ok(v.errors.some((e) => /SHA mismatch/.test(e)));
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
