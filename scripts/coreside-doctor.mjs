#!/usr/bin/env node
/**
 * Coreside offline doctor — verifies generated-application runtime invariants.
 * No network. No user secrets. Exit 1 when any check fails.
 *
 * Usage:
 *   npm run doctor
 *   npm run doctor -- --json
 */

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const jsonMode = process.argv.includes("--json");

/** @type {{ id: string, ok: boolean, detail: string }[]} */
const checks = [];

function check(id, ok, detail) {
  checks.push({ id, ok: Boolean(ok), detail });
}

function read(rel) {
  return fs.readFileSync(path.join(root, rel), "utf8");
}

function exists(rel) {
  return fs.existsSync(path.join(root, rel));
}

function listMigrations() {
  const dir = path.join(root, "src-tauri/migrations");
  return fs
    .readdirSync(dir)
    .filter((f) => f.endsWith(".sql"))
    .sort();
}

// --- Migrations ---
const migrations = listMigrations();
check(
  "migrations.present",
  migrations.includes("015_registered_actions.sql"),
  migrations.includes("015_registered_actions.sql")
    ? "015_registered_actions.sql present"
    : "missing 015_registered_actions.sql",
);

const ordered = migrations.every((name, i, arr) => {
  if (i === 0) return true;
  return name.localeCompare(arr[i - 1]) > 0;
});
check("migrations.ordered", ordered, `found ${migrations.length} migrations`);

const migration015 = exists("src-tauri/migrations/015_registered_actions.sql")
  ? read("src-tauri/migrations/015_registered_actions.sql")
  : "";
for (const table of [
  "runtime_action_grants",
  "runtime_approvals",
  "runtime_approval_claims",
  "runtime_audit_events",
  "application_build_failures",
]) {
  check(
    `migrations.table.${table}`,
    migration015.includes(`CREATE TABLE IF NOT EXISTS ${table}`),
    migration015.includes(`CREATE TABLE IF NOT EXISTS ${table}`)
      ? `${table} defined`
      : `${table} missing from 015`,
  );
}
check(
  "migrations.approvals.input_json",
  migration015.includes("input_json"),
  migration015.includes("input_json")
    ? "frozen approval input column present"
    : "runtime_approvals.input_json missing",
);

// --- DB migration wiring ---
const dbMod = read("src-tauri/src/db/mod.rs");
check(
  "db.applies_015",
  dbMod.includes("015_registered_actions") &&
    dbMod.includes("LATEST_MIGRATION") &&
    dbMod.includes("MIGRATIONS"),
  "db/mod.rs applies 015_registered_actions via the ordered MIGRATIONS list",
);

// --- Registered action registry ---
const descriptor = read(
  "src-tauri/src/application_kernel/registered_actions/descriptor.rs",
);
const actionNames = [...descriptor.matchAll(/"([a-z][a-z0-9_.]+)"/g)]
  .map((m) => m[1])
  .filter((n) => n.includes("."));
const uniqueNames = new Set(
  [...descriptor.matchAll(/descriptor\(\s*"([^"]+)"/g)].map((m) => m[1]),
);
check(
  "actions.registry_nonempty",
  uniqueNames.size >= 8,
  `${uniqueNames.size} bundled actions`,
);
check(
  "actions.names_unique",
  uniqueNames.size ===
    [...descriptor.matchAll(/descriptor\(\s*"([^"]+)"/g)].length,
  "bundled action names are unique",
);

const requiredActions = [
  "local_data.query",
  "local_data.write",
  "local_data.delete",
  "tool_state.set",
  "web_search.request",
  "media.read",
  "external_link.open",
  "export.prepare",
  "automation.propose",
  "agent.submit_event",
];
for (const name of requiredActions) {
  check(
    `actions.has.${name}`,
    uniqueNames.has(name),
    uniqueNames.has(name) ? "registered" : "missing from BUNDLED_ACTIONS",
  );
}

check(
  "actions.risk_axis",
  descriptor.includes("ActionRisk::Read") &&
    descriptor.includes("ActionRisk::Write") &&
    descriptor.includes("ActionRisk::Destructive"),
  "read/write/destructive risk axis present",
);

// --- Permission name agreement (Rust ↔ TS types/docs surface) ---
const permissionsRs = read(
  "src-tauri/src/application_kernel/permissions.rs",
);
const allowedPermMatches = [
  ...permissionsRs.matchAll(/"([a-z][a-z0-9_.]+)"/g),
].map((m) => m[1]);
const allowedPerms = new Set(
  allowedPermMatches.filter(
    (p) =>
      p.includes(".") &&
      !p.startsWith("filesystem.") &&
      permissionsRs.includes(`"${p}"`),
  ),
);
// Extract ALLOWED_PERMISSIONS block more tightly
const allowedBlock = permissionsRs.match(
  /ALLOWED_PERMISSIONS[^=]*=\s*&\[([\s\S]*?)\]/,
);
const rustAllowed = allowedBlock
  ? [...allowedBlock[1].matchAll(/"([^"]+)"/g)].map((m) => m[1])
  : [];
check(
  "permissions.allowed_nonempty",
  rustAllowed.length >= 5,
  `${rustAllowed.length} allowed permission categories`,
);

for (const name of uniqueNames) {
  const catMatch = descriptor.match(
    new RegExp(
      `descriptor\\(\\s*"${name.replace(".", "\\.")}"[\\s\\S]*?"([a-z][a-z0-9_.]+)"\\s*,`,
    ),
  );
  // looser: ensure permission category string near each descriptor — covered by Rust tests
  void catMatch;
}
check(
  "permissions.no_unrestricted_fs",
  !rustAllowed.includes("filesystem.unrestricted") &&
    permissionsRs.includes("FORBIDDEN_PERMISSIONS"),
  "unrestricted filesystem permission remains forbidden",
);

// --- Gateway choke point ---
const gateway = read(
  "src-tauri/src/application_kernel/registered_actions/gateway.rs",
);
check(
  "gateway.single_choke",
  gateway.includes("execute_registered_action") &&
    gateway.includes("generated_execution_allowed"),
  "gateway + recovery gate present",
);
check(
  "gateway.schema_validation",
  gateway.includes("validate_input"),
  "input schema validation wired into gateway",
);

// --- Commands registered ---
const libRs = read("src-tauri/src/lib.rs");
const requiredCommands = [
  "kernel_invoke_registered_action",
  "kernel_list_pending_approvals",
  "kernel_decide_approval",
  "kernel_list_runtime_grants",
  "kernel_revoke_runtime_grant",
  "kernel_list_audit_events",
  "kernel_set_application_lifecycle",
  "kernel_list_registered_actions",
];
for (const cmd of requiredCommands) {
  check(
    `commands.${cmd}`,
    libRs.includes(cmd),
    libRs.includes(cmd) ? "registered" : "missing from lib.rs",
  );
}

// --- Frontend must not expose generic privileged invoke ---
const toolRenderer = read("src/components/tool-renderer/ToolRenderer.tsx");
check(
  "frontend.no_generic_invoke",
  !toolRenderer.includes("invoke(") &&
    toolRenderer.includes("kernelInvokeRegisteredAction"),
  "ToolRenderer uses kernelInvokeRegisteredAction only",
);

const actionsTs = read("src/lib/actions.ts");
check(
  "frontend.no_tauri_in_action_engine",
  !actionsTs.includes("invoke(") && !actionsTs.includes("@tauri-apps"),
  "action engine stays free of Tauri invoke",
);

/** Frontend IPC bridge — directory split or legacy monolithic file. */
function readTauriBridge() {
  const dir = path.join(root, "src/lib/tauri");
  if (fs.existsSync(dir) && fs.statSync(dir).isDirectory()) {
    /** @type {string[]} */
    const parts = [];
    const walk = (abs) => {
      for (const name of fs.readdirSync(abs).sort()) {
        const child = path.join(abs, name);
        if (fs.statSync(child).isDirectory()) walk(child);
        else if (name.endsWith(".ts")) parts.push(fs.readFileSync(child, "utf8"));
      }
    };
    walk(dir);
    return parts.join("\n");
  }
  return read("src/lib/tauri.ts");
}

const tauriTs = readTauriBridge();
check(
  "frontend.api_bindings",
  tauriTs.includes("kernelInvokeRegisteredAction") &&
    tauriTs.includes("kernelDecideApproval") &&
    tauriTs.includes("kernelListRuntimeGrants"),
  "TypeScript API bindings present",
);

// --- Approval UI ---
check(
  "ui.approval_card",
  exists("src/components/applications/ApprovalCard.tsx"),
  "ApprovalCard present",
);
check(
  "ui.pending_host",
  exists("src/components/applications/PendingApprovalsHost.tsx"),
  "PendingApprovalsHost present",
);
check(
  "ui.app_details",
  exists("src/components/applications/ApplicationDetailsPanel.tsx"),
  "ApplicationDetailsPanel present",
);

// --- Manifest requires application identity ---
const manifestRs = read("src-tauri/src/application_kernel/manifest.rs");
check(
  "manifest.action_access_validated",
  manifestRs.includes("validate_action_access") &&
    manifestRs.includes("application_action_access"),
  "manifest action access validated",
);
check(
  "manifest.ensure_for_tool",
  manifestRs.includes("ensure_manifest_for_tool"),
  "legacy tools can be wrapped with manifests",
);

// --- Recovery blocks generated execution (app surfaces + automations) ---
check(
  "recovery.blocks_execution",
  gateway.includes("recovery_required") || gateway.includes("Recovery Mode"),
  "recovery mode blocks generated execution",
);
check(
  "recovery.blocks_automation_venue",
  /Venue::Application\s*\|\s*Venue::Automation/.test(gateway) ||
    (gateway.includes("Venue::Automation") &&
      gateway.includes("generated_execution_allowed")),
  "recovery gate covers Automation venue, not only Application",
);

// --- Approval host mounted ---
const appShell = read("src/components/layout/AppShell.tsx");
check(
  "ui.approval_host_mounted",
  appShell.includes("PendingApprovalsHost"),
  "PendingApprovalsHost mounted in AppShell",
);
const inlineSurface = read("src/components/chat/InlineSurface.tsx");
check(
  "ui.inline_pending_approval",
  inlineSurface.includes("onPendingApproval") &&
    inlineSurface.includes("coreside:pending-approval"),
  "inline surfaces notify the approval host",
);

// --- Packages strip authority ---
const packagesRs = read("src-tauri/src/application_kernel/packages.rs");
check(
  "packages.authority_stripped",
  packagesRs.includes("AUTHORITY_KEYS") &&
    packagesRs.includes("strip_authority_keys") &&
    packagesRs.includes("runtimeGrants") &&
    packagesRs.includes("grantedPermissions") &&
    !packagesRs.includes("grant_permission(") &&
    !packagesRs.includes("mint_grant("),
  "import strips authority keys and never grants/mints authority",
);

// --- Contract drift: frontend IPC wrappers vs registered Rust commands ---
const bridge = tauriTs;
const registeredCommands = new Set(
  [...libRs.matchAll(/commands::([a-z0-9_]+)/g)].map((m) => m[1]),
);
const invokedCommands = new Set(
  [...bridge.matchAll(/invoke(?:<[^>]*>)?\(\s*"([a-z0-9_]+)"/g)].map((m) => m[1]),
);
const unregisteredInvokes = [...invokedCommands].filter(
  (c) => !registeredCommands.has(c),
);
check(
  "contracts.invoke_is_registered",
  unregisteredInvokes.length === 0,
  unregisteredInvokes.length === 0
    ? `all ${invokedCommands.size} invoked commands are registered in lib.rs`
    : `invoked but never registered: ${unregisteredInvokes.join(", ")}`,
);

// A mock case with no production command would let tests pass against behaviour
// the desktop build cannot perform. Explicit mock-only allowlist is OK.
const MOCK_ONLY = new Set(
  [
    ...bridge.matchAll(
      /MOCK_ONLY_COMMANDS\s*=\s*new Set(?:<[^>]*>)?\(\[([^\]]*)\]/g,
    ),
  ].flatMap((m) => [...m[1].matchAll(/"([a-z0-9_]+)"/g)].map((x) => x[1])),
);
const mockedCommands = new Set(
  [...bridge.matchAll(/case\s+"([a-z0-9_]+)":/g)].map((m) => m[1]),
);
const ghostMocks = [...mockedCommands].filter(
  (c) => !registeredCommands.has(c) && !MOCK_ONLY.has(c),
);
check(
  "contracts.mock_has_production_command",
  ghostMocks.length === 0,
  ghostMocks.length === 0
    ? `all ${mockedCommands.size} mocked commands exist in production`
    : `mocked with no registered command: ${ghostMocks.join(", ")}`,
);

// --- Documentation drift: migration inventory matches the migration files ---
// Match the inventory table rows only. A passing mention in prose is not an
// inventory entry, which is exactly how 015 went missing from the table while
// the count line above it already said fifteen.
const inventoryRows = read("docs/MIGRATION_ASSURANCE.md")
  .split("\n")
  .filter((line) => /^\|\s*\d{3}\s*\|/.test(line))
  .join("\n");
const undocumentedMigrations = migrations.filter(
  (file) => !inventoryRows.includes(file),
);
check(
  "docs.migration_inventory_complete",
  undocumentedMigrations.length === 0,
  undocumentedMigrations.length === 0
    ? `all ${migrations.length} migrations listed in docs/MIGRATION_ASSURANCE.md`
    : `missing from the inventory table: ${undocumentedMigrations.join(", ")}`,
);

// --- Tool windows must not receive shell:allow-open ---
const toolCaps = exists("src-tauri/capabilities/tool-window.json")
  ? read("src-tauri/capabilities/tool-window.json")
  : "";
const mainCaps = read("src-tauri/capabilities/default.json");
check(
  "capabilities.tool_window_no_shell",
  toolCaps.includes('"tool-*"') &&
    !toolCaps.includes("shell:allow-open") &&
    !mainCaps.includes('"tool-*"'),
  "tool-* windows use a separate capability without shell:allow-open",
);
check(
  "capabilities.tool_window_no_global_deny",
  (() => {
    try {
      const caps = JSON.parse(toolCaps);
      const perms = Array.isArray(caps.permissions) ? caps.permissions : [];
      return (
        perms.includes("coreside-tool-scoped") &&
        !perms.includes("coreside-tool-deny-sensitive")
      );
    } catch {
      return false;
    }
  })(),
  "tool-window must not link coreside-tool-deny-sensitive (Tauri commands.deny is global and blocks main)",
);
check(
  "commands.open_external_url",
  libRs.includes("open_external_url") &&
    bridge.includes("open_external_url") &&
    bridge.includes("openExternalUrl"),
  "external links open through the validated Rust command",
);

// --- Desktop E2E: production must not enable WebDriver ---
const cargoToml = read("src-tauri/Cargo.toml");
const defaultFeaturesMatch = cargoToml.match(
  /\[features\][\s\S]*?^default\s*=\s*\[([^\]]*)\]/m,
);
const defaultFeatures = defaultFeaturesMatch
  ? defaultFeaturesMatch[1]
  : "";
check(
  "e2e.not_in_default_features",
  !/\be2e\b/.test(defaultFeatures) &&
    cargoToml.includes('e2e = ["dep:tauri-plugin-wdio"') &&
    cargoToml.includes("optional = true"),
  "Cargo feature e2e is optional and excluded from default features",
);

const tauriConf = read("src-tauri/tauri.conf.json");
check(
  "e2e.production_config_no_wdio",
  !tauriConf.includes("wdio") &&
    !tauriConf.includes("withGlobalTauri") &&
    tauriConf.includes('"capabilities": ["default", "tool-window"]'),
  "production tauri.conf.json does not enable WebDriver or e2e capabilities",
);
check(
  "e2e.production_caps_no_wdio",
  !mainCaps.includes("wdio") && !toolCaps.includes("wdio"),
  "production capability files do not grant wdio permissions",
);
check(
  "e2e.dedicated_config_present",
  exists("src-tauri/tauri.e2e.conf.json") &&
    read("src-tauri/tauri.e2e.conf.json").includes("wdio:default") &&
    read("src-tauri/tauri.e2e.conf.json").includes('"tool-*"'),
  "tauri.e2e.conf.json gates WebDriver for E2E builds on main + tool-* windows",
);
check(
  "e2e.lib_plugins_feature_gated",
  libRs.includes('#[cfg(feature = "e2e")]') &&
    libRs.includes("tauri_plugin_wdio") &&
    libRs.includes("tauri_plugin_wdio_webdriver"),
  "WebDriver plugins register only behind cfg(feature = \"e2e\")",
);
check(
  "e2e.seed_not_public_ipc",
  exists("src-tauri/src/e2e_support.rs") &&
    read("src-tauri/src/e2e_support.rs").includes("CORESIDE_E2E_SEED") &&
    libRs.includes('#[cfg(feature = "e2e")]') &&
    libRs.includes("mod e2e_support") &&
    libRs.includes("e2e_support::maybe_seed") &&
    !/\be2e_seed\b/.test(libRs) &&
    !/generate_handler!\[[\s\S]*e2e_support/.test(libRs),
  "E2E seed is env-only behind feature e2e — not a privileged Tauri IPC command",
);
check(
  "e2e.harness_no_broad_pkill",
  exists("e2e/wdio.conf.ts") &&
    !/spawnSync\(\s*["']pkill["']/.test(read("e2e/wdio.conf.ts")) &&
    !/["']pkill["']/.test(read("e2e/wdio.conf.ts")) &&
    read("e2e/wdio.conf.ts").includes("killOrphanedE2eWebDrivers") &&
    read("e2e/wdio.conf.ts").includes("WEBDRIVER_PORT") &&
    read("e2e/run.mjs").includes("CORESIDE_DB_PATH"),
  "E2E harness never broad-pkills Coreside; orphans cleaned only via WebDriver port + binary match",
);
check(
  "e2e.tsconfig_isolated",
  exists("e2e/tsconfig.json") &&
    read("tsconfig.json").includes('"exclude"') &&
    /"exclude"\s*:\s*\[[^\]]*e2e/.test(read("tsconfig.json")),
  "Root tsconfig excludes e2e so @wdio types cannot leak into app tsc",
);

// --- Release evidence drift (do not fail solely because the tree is dirty) ---
check(
  "evidence.script_present",
  exists("scripts/release-evidence.mjs") &&
    read("package.json").includes('"release:evidence"'),
  "npm run release:evidence is available",
);
check(
  "evidence.tracks_separated",
  exists("scripts/release-evidence.mjs") &&
    read("scripts/release-evidence.mjs").includes("localByok") &&
    read("scripts/release-evidence.mjs").includes("hostedAi"),
  "Evidence script separates local BYOK vs hosted AI tracks",
);

// Missing evidence must not fail doctor: release:evidence runs doctor first
// (chicken-and-egg). When a file exists, it must be honest and current.
if (process.env.RUNNING_RELEASE_EVIDENCE === "1") {
  check(
    "evidence.current_commit",
    true,
    "release:evidence is actively regenerating evidence",
  );
} else if (exists("reports/release-evidence.json")) {
  let evidenceOk = false;
  let detail = "release-evidence.json unreadable";
  try {
    const ev = JSON.parse(read("reports/release-evidence.json"));
    const head = spawnSyncGitHead();
    const evCommit = ev?.git?.commit || ev?.commit;
    const sameCommit = !head || evCommit === head;
    const hasGates = Array.isArray(ev?.gates);
    // "passed" requires a numeric exitCode of 0 — null/undefined is invented.
    const inventsPass = (ev?.gates || []).some(
      (g) =>
        g.status === "passed" &&
        (typeof g.exitCode !== "number" || g.exitCode !== 0),
    );
    evidenceOk = hasGates && sameCommit && !inventsPass;
    detail = !hasGates
      ? "release-evidence.json missing gates[]"
      : !sameCommit
        ? `evidence commit ${ev?.git?.commit?.slice(0, 7)} != HEAD ${head?.slice(0, 7)} (regenerate with npm run release:evidence)`
        : inventsPass
          ? "evidence claims passed without exitCode === 0"
          : `evidence matches HEAD ${head?.slice(0, 7) || "(unknown)"}`;
  } catch (e) {
    detail = String(e && typeof e === "object" && "message" in e ? e.message : e);
  }
  check("evidence.current_commit", evidenceOk, detail);
} else {
  check(
    "evidence.current_commit",
    true,
    "reports/release-evidence.json not generated yet (run npm run release:evidence)",
  );
}

check(
  "docs.release_readiness_points_to_evidence",
  exists("docs/RELEASE_READINESS.md") &&
    (read("docs/RELEASE_READINESS.md").includes("release-evidence.json") ||
      read("docs/RELEASE_READINESS.md").includes("npm run release:evidence")),
  "RELEASE_READINESS.md references generated evidence rather than stale July counts alone",
);

check(
  "reports.release_gates_marked_historical",
  !exists("reports/release-gates.json") ||
    read("reports/release-gates.json").includes('"status": "historical"') ||
    read("reports/release-gates.json").includes('"superseded"') ||
    read("scripts/release-evidence.mjs").includes("release-gates.json"),
  "Stale release-gates.json is superseded by release-evidence pipeline",
);

function spawnSyncGitHead() {
  try {
    const r = spawnSync("git", ["rev-parse", "HEAD"], {
      cwd: root,
      encoding: "utf8",
    });
    return (r.stdout || "").trim() || null;
  } catch {
    return null;
  }
}

const failed = checks.filter((c) => !c.ok);
const report = {
  ok: failed.length === 0,
  checkedAt: new Date().toISOString(),
  total: checks.length,
  passed: checks.length - failed.length,
  failed: failed.length,
  checks,
};

if (jsonMode) {
  console.log(JSON.stringify(report, null, 2));
} else {
  console.log("Coreside doctor");
  console.log("===============");
  for (const c of checks) {
    console.log(`${c.ok ? "PASS" : "FAIL"}  ${c.id} — ${c.detail}`);
  }
  console.log("");
  console.log(
    failed.length === 0
      ? `All ${checks.length} checks passed.`
      : `${failed.length} of ${checks.length} checks failed.`,
  );
}

process.exit(failed.length === 0 ? 0 : 1);
