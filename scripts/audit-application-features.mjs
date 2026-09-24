#!/usr/bin/env node
/**
 * Lightweight application feature inventory from known routes/panels.
 * Readiness guesses come from reports/readiness-ladder.json when present.
 *
 * Writes: reports/application-feature-inventory.json
 * Usage:  npm run audit:application-features
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const reportsDir = path.join(root, "reports");

function git(args) {
  const r = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  return (r.stdout || "").trim();
}

function readJson(rel) {
  const p = path.join(root, rel);
  if (!fs.existsSync(p)) return null;
  try {
    return JSON.parse(fs.readFileSync(p, "utf8"));
  } catch {
    return null;
  }
}

function exists(rel) {
  return fs.existsSync(path.join(root, rel));
}

/** Known consumer surfaces — static catalog, not a live route scanner. */
const SURFACES = [
  {
    id: "sidebar",
    name: "Sidebar",
    kind: "shell",
    entry: "src/components/sidebar/Sidebar.tsx",
    readinessIds: [],
  },
  {
    id: "chat",
    name: "Chat",
    kind: "view",
    viewKind: "chat",
    entry: "src/components/chat/ChatPanel.tsx",
    readinessIds: ["true-streaming", "channel-scoped-streaming", "queue-ui"],
  },
  {
    id: "projects",
    name: "Projects",
    kind: "view",
    viewKind: "projects",
    entry: "src/components/projects/ProjectList.tsx",
    readinessIds: [],
  },
  {
    id: "tools",
    name: "Tools",
    kind: "canvas",
    // Tools open the canvas while remaining on chat view (selectTool).
    viewKind: "chat",
    entry: "src/components/tool-canvas/ToolCanvas.tsx",
    readinessIds: ["progressive-preview", "history-replay"],
  },
  {
    id: "media",
    name: "Media Library",
    kind: "view",
    viewKind: "media",
    entry: "src/components/media/MediaLibrary.tsx",
    readinessIds: ["attachment-authorization", "attachment-gc"],
  },
  {
    id: "automations",
    name: "Automations",
    kind: "view",
    viewKind: "automations",
    entry: "src/components/automations/AutomationsPanel.tsx",
    readinessIds: [],
  },
  {
    id: "settings",
    name: "Settings",
    kind: "view",
    viewKind: "settings",
    entry: "src/components/settings/SettingsPanel.tsx",
    readinessIds: ["wallpaper-atomic-settings"],
    categoriesFrom: "src/lib/settings-categories.ts",
  },
  {
    id: "recovery",
    name: "Recovery",
    kind: "panel",
    entry: "src/components/recovery/BootstrapRecoveryScreen.tsx",
    settingsEntry: "src/components/settings/RecoverySettings.tsx",
    readinessIds: [],
  },
];

function loadSettingsCategories() {
  const tsPath = path.join(root, "src/lib/settings-categories.ts");
  if (fs.existsSync(tsPath)) {
    const text = fs.readFileSync(tsPath, "utf8");
    const block = text.match(
      /SETTINGS_CATEGORIES:\s*readonly\s*SettingsCategory\[\]\s*=\s*\[([\s\S]*?)\]\s*as\s*const/,
    );
    if (block) {
      const matches = [...block[1].matchAll(/id:\s*["']([^"']+)["']/g)].map(
        (m) => m[1],
      );
      if (matches.length > 0) {
        return matches;
      }
    }
  }
  return [
    "general",
    "appearance",
    "ai-access",
    "agent",
    "search",
    "privacy",
    "data",
    "accessibility",
    "advanced",
    "help-learning",
    "about",
    "added",
  ];
}

const SETTINGS_CATEGORIES = loadSettingsCategories();

const ladder = readJson("reports/readiness-ladder.json");
const ladderById = new Map(
  (ladder?.features || []).map((f) => [f.id, f.state]),
);

function guessState(surface) {
  const present = exists(surface.entry);
  if (!present) {
    return {
      readinessGuess: "Absent",
      notes: `Entry missing: ${surface.entry}`,
    };
  }
  const states = surface.readinessIds
    .map((id) => ladderById.get(id))
    .filter(Boolean);
  if (states.length === 0) {
    return {
      readinessGuess: "Integrated – Not Verified",
      notes: ladder
        ? "Present in tree; no matching readiness-ladder feature ids."
        : "Present in tree; readiness-ladder.json missing — guess only.",
    };
  }
  // Lowest-confidence among linked ladder states (honest aggregate).
  const order = [
    "Absent",
    "Scaffolded",
    "Integrated – Not Verified",
    "Unit Verified",
    "Desktop Verified",
    "Packaged Verified",
    "Cross-Platform Verified",
    "Human Accepted",
    "Blocked by External Prerequisite",
    "Deliberately Deferred",
    "Rejected by Product Architecture",
  ];
  let lowest = states[0];
  for (const s of states) {
    if (order.indexOf(s) < order.indexOf(lowest)) lowest = s;
  }
  return {
    readinessGuess: lowest,
    notes: `Derived from readiness-ladder: ${surface.readinessIds.join(", ")}`,
    linkedLadderStates: Object.fromEntries(
      surface.readinessIds.map((id) => [id, ladderById.get(id) ?? null]),
    ),
  };
}

const features = SURFACES.map((s) => {
  const guess = guessState(s);
  return {
    id: s.id,
    name: s.name,
    kind: s.kind,
    viewKind: s.viewKind ?? null,
    entry: s.entry,
    entryPresent: exists(s.entry),
    ...(s.settingsEntry
      ? { settingsEntry: s.settingsEntry, settingsEntryPresent: exists(s.settingsEntry) }
      : {}),
    ...guess,
  };
});

const report = {
  schemaVersion: 1,
  product: "Coreside",
  generatedAt: new Date().toISOString(),
  commit: git(["rev-parse", "HEAD"]) || "unknown",
  dirty: git(["status", "--porcelain"]).length > 0,
  command: "audit:application-features",
  evidenceLevel: "Scaffolded",
  note: "Static catalog of known routes/panels with readiness guesses — not Desktop/Packaged Verified.",
  readinessLadderPresent: Boolean(ladder),
  readinessLadderCommit: ladder?.commit ?? null,
  settingsCategories: SETTINGS_CATEGORIES,
  features,
};

fs.mkdirSync(reportsDir, { recursive: true });
const outPath = path.join(reportsDir, "application-feature-inventory.json");
fs.writeFileSync(outPath, JSON.stringify(report, null, 2) + "\n");
console.log(
  JSON.stringify(
    { outPath, featureCount: features.length, readinessLadderPresent: Boolean(ladder) },
    null,
    2,
  ),
);
