#!/usr/bin/env node
/**
 * Unit checks for the storage detector classification helpers.
 * Run: node --test scripts/storage-audit.test.mjs
 */

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import {
  discoverInventory,
  formatBytes,
  isCursorSandboxCargoTarget,
  isProtectedPath,
  parseArgs,
} from "./storage-audit.mjs";

test("parseArgs understands smart/clean/threshold", () => {
  assert.equal(parseArgs(["--smart"]).mode, "smart");
  assert.equal(parseArgs(["--mode=clean"]).mode, "clean");
  assert.equal(parseArgs(["--threshold-gb=8"]).thresholdGb, 8);
  assert.throws(() => parseArgs(["--mode=explode"]));
});

test("formatBytes handles zero and gigabytes", () => {
  assert.equal(formatBytes(0), "0 B");
  assert.match(formatBytes(5 * 1024 ** 3), /5\.00 GB/);
});

test("sandbox cargo-target detection", () => {
  const p = path.join(
    os.tmpdir(),
    "cursor-sandbox-cache",
    "abc123",
    "cargo-target",
  );
  assert.equal(isCursorSandboxCargoTarget(p), true);
  assert.equal(isCursorSandboxCargoTarget("/tmp/random/cargo-target"), false);
});

test("protected paths never include source or services", () => {
  const root = "/tmp/coreside-fake-root";
  assert.equal(isProtectedPath(path.join(root, "src"), root), true);
  assert.equal(isProtectedPath(path.join(root, "src-tauri/src"), root), true);
  assert.equal(
    isProtectedPath(path.join(root, "src-tauri/migrations"), root),
    true,
  );
  assert.equal(isProtectedPath(path.join(root, "services/crawl4ai"), root), true);
  assert.equal(isProtectedPath(path.join(root, "src-tauri/target"), root), false);
  assert.equal(
    isProtectedPath(path.join(root, "node_modules/.vite"), root),
    false,
  );
  assert.equal(isProtectedPath(path.join(root, "node_modules"), root), true);
});

test("discoverInventory marks in-repo target cleanable and services protected", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "coreside-storage-"));
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "coreside-tmp-"));
  try {
    fs.mkdirSync(path.join(root, "src-tauri", "target"), { recursive: true });
    fs.mkdirSync(path.join(root, "services", "crawl4ai"), { recursive: true });
    fs.mkdirSync(path.join(root, "node_modules", ".vite"), { recursive: true });
    const sandboxTarget = path.join(
      tmpDir,
      "cursor-sandbox-cache",
      "deadbeefcafe",
      "cargo-target",
    );
    fs.mkdirSync(sandboxTarget, { recursive: true });
    fs.writeFileSync(path.join(sandboxTarget, "x"), "y");

    const items = discoverInventory({
      root,
      tmpDir,
      env: { CARGO_TARGET_DIR: sandboxTarget },
    });

    const target = items.find((i) => i.id === "rust-target");
    assert.ok(target);
    assert.equal(target.safeToClean, true);

    const services = items.find((i) => i.id === "services");
    assert.ok(services);
    assert.equal(services.safeToClean, false);

    const sandbox = items.filter((i) => i.safeToClean && i.path === sandboxTarget);
    assert.ok(sandbox.length >= 1, "sandbox cargo-target should be cleanable");

    // No inventory row should claim source is cleanable.
    for (const item of items) {
      if (item.safeToClean) {
        assert.equal(
          isProtectedPath(item.path, root) && item.kind !== "logs",
          false,
          `cleanable item leaked protected path: ${item.path}`,
        );
      }
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
});
