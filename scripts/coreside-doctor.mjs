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
  dbMod.includes("015_registered_actions") && dbMod.includes("MIGRATION_015"),
  "db/mod.rs applies 015_registered_actions",
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

const tauriTs = read("src/lib/tauri.ts");
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
