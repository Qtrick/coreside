#!/usr/bin/env node
/**
 * Typecheck deployable Supabase Edge Function entrypoints.
 *
 * Requires Deno on PATH. Does not silently skip — exits non-zero when Deno is absent.
 *
 * Usage: npm run typecheck:edge-functions
 */
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const functionsDir = path.join(root, "supabase", "functions");

const ENTRYPOINTS = [
  "ai-gateway/index.ts",
  "billing-checkout/index.ts",
  "billing-portal/index.ts",
  "search-gateway/index.ts",
  "stripe-webhook/index.ts",
];

const deno = spawnSync("deno", ["--version"], { encoding: "utf8" });
if (deno.error || deno.status !== 0) {
  console.error(
    "typecheck:edge-functions FAILED: Deno is not installed or not on PATH.",
  );
  console.error(
    "Install Deno (https://docs.deno.com/runtime/getting_started/installation/) and re-run.",
  );
  console.error(
    "Helper-library Vitest suites do not prove deployable Deno.serve entrypoints compile.",
  );
  process.exit(1);
}

console.log(deno.stdout.trim().split("\n")[0] || "deno present");

let failed = 0;
for (const rel of ENTRYPOINTS) {
  const abs = path.join(functionsDir, rel);
  if (!fs.existsSync(abs)) {
    console.error(`MISSING entrypoint: ${rel}`);
    failed += 1;
    continue;
  }
  const r = spawnSync(
    "deno",
    ["check", "--config", path.join(functionsDir, "deno.json"), abs],
    { cwd: root, encoding: "utf8" },
  );
  if (r.status !== 0) {
    console.error(`FAIL ${rel}`);
    if (r.stdout) console.error(r.stdout);
    if (r.stderr) console.error(r.stderr);
    failed += 1;
  } else {
    console.log(`ok ${rel}`);
  }
}

if (failed > 0) {
  console.error(`typecheck:edge-functions: ${failed} entrypoint(s) failed`);
  process.exit(1);
}
console.log(`typecheck:edge-functions: ${ENTRYPOINTS.length} entrypoints ok`);
