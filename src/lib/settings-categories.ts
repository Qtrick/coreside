/**
 * Consumer Settings category model + search index.
 * Wallpaper templates stay under Added Settings (not a Base category).
 */

export type SettingsCategoryId =
  | "general"
  | "appearance"
  | "ai-access"
  | "agent"
  | "search"
  | "privacy"
  | "data"
  | "accessibility"
  | "advanced"
  | "about"
  | "added";

export type SettingsCategoryGroup = "coreside" | "added";

export type SettingsCategory = {
  id: SettingsCategoryId;
  label: string;
  group: SettingsCategoryGroup;
  description: string;
};

export const SETTINGS_CATEGORIES: readonly SettingsCategory[] = [
  {
    id: "general",
    label: "General",
    group: "coreside",
    description: "Everyday window and launch behavior.",
  },
  {
    id: "appearance",
    label: "Appearance",
    group: "coreside",
    description: "Theme and Dock icon. Wallpapers live under Added Settings.",
  },
  {
    id: "ai-access",
    label: "AI Access",
    group: "coreside",
    description: "How Coreside connects to AI on this computer.",
  },
  {
    id: "agent",
    label: "Agent",
    group: "coreside",
    description: "How the assistant shows progress and Action Log detail.",
  },
  {
    id: "search",
    label: "Search & Research",
    group: "coreside",
    description: "Web discovery, crawl, budgets, and research cache.",
  },
  {
    id: "privacy",
    label: "Privacy & Security",
    group: "coreside",
    description: "What stays local and what you can revoke.",
  },
  {
    id: "data",
    label: "Data & Storage",
    group: "coreside",
    description: "Local chats, tools, and clear controls.",
  },
  {
    id: "accessibility",
    label: "Accessibility",
    group: "coreside",
    description: "Motion and inclusive use.",
  },
  {
    id: "advanced",
    label: "Advanced",
    group: "coreside",
    description: "Recovery and application permissions.",
  },
  {
    id: "about",
    label: "About",
    group: "coreside",
    description: "Coreside version and identity.",
  },
  {
    id: "added",
    label: "Added Settings",
    group: "added",
    description: "Wallpapers and preferences created by your tools.",
  },
] as const;

export const DEFAULT_SETTINGS_CATEGORY: SettingsCategoryId = "general";

const CATEGORY_IDS = new Set(
  SETTINGS_CATEGORIES.map((c) => c.id),
) as ReadonlySet<string>;

export function isSettingsCategoryId(value: unknown): value is SettingsCategoryId {
  return typeof value === "string" && CATEGORY_IDS.has(value);
}

export function normalizeSettingsCategoryId(
  value: unknown,
  fallback: SettingsCategoryId = DEFAULT_SETTINGS_CATEGORY,
): SettingsCategoryId {
  return isSettingsCategoryId(value) ? value : fallback;
}

export type SettingsSearchEntry = {
  id: string;
  categoryId: SettingsCategoryId;
  label: string;
  keywords: string[];
};

/** Searchable labels that navigate to the owning category (not duplicate controls). */
export const SETTINGS_SEARCH_INDEX: readonly SettingsSearchEntry[] = [
  {
    id: "adaptive-window",
    categoryId: "general",
    label: "Adaptive window sizing",
    keywords: ["window", "expand", "smart", "ask first", "monitor"],
  },
  {
    id: "theme",
    categoryId: "appearance",
    label: "Theme",
    keywords: ["light", "dark", "system", "appearance"],
  },
  {
    id: "dock-icon",
    categoryId: "appearance",
    label: "Dock icon",
    keywords: ["macos", "tile", "brand"],
  },
  {
    id: "wallpaper-link",
    categoryId: "added",
    label: "Wallpapers",
    keywords: ["wallpaper", "templates", "transparency", "live"],
  },
  {
    id: "api-key",
    categoryId: "ai-access",
    label: "API key",
    keywords: ["byok", "provider", "openai", "anthropic", "gemini", "key"],
  },
  {
    id: "ai-provider",
    categoryId: "ai-access",
    label: "AI provider",
    keywords: ["model", "connection", "hosted", "local", "coreside ai"],
  },
  {
    id: "action-log",
    categoryId: "agent",
    label: "Action Log",
    keywords: ["agent", "progress", "steps"],
  },
  {
    id: "exa",
    categoryId: "search",
    label: "Web search",
    keywords: ["exa", "research", "crawl", "budget", "cache"],
  },
  {
    id: "privacy",
    categoryId: "privacy",
    label: "Privacy",
    keywords: ["local", "security", "permissions", "grants", "revoke"],
  },
  {
    id: "backup",
    categoryId: "data",
    label: "Backup and data",
    keywords: ["clear", "storage", "conversations", "tools", "export"],
  },
  {
    id: "reduced-motion",
    categoryId: "accessibility",
    label: "Reduced motion",
    keywords: ["accessibility", "animation", "prefers-reduced-motion"],
  },
  {
    id: "recovery",
    categoryId: "advanced",
    label: "Recovery",
    keywords: ["recovery mode", "restore", "last known good"],
  },
  {
    id: "runtime-permissions",
    categoryId: "advanced",
    label: "App permissions",
    keywords: ["grants", "revoke", "runtime", "application permissions"],
  },
  {
    id: "version",
    categoryId: "about",
    label: "Version",
    keywords: ["about", "coreside"],
  },
  {
    id: "templates",
    categoryId: "added",
    label: "Templates",
    keywords: ["wallpaper", "added settings", "tool settings"],
  },
];

export type SettingsSearchHit = SettingsSearchEntry & { score: number };

export function matchSettingsSearch(
  query: string,
  entries: readonly SettingsSearchEntry[] = SETTINGS_SEARCH_INDEX,
): SettingsSearchHit[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];

  const parts = q.split(/\s+/).filter(Boolean);
  const hits: SettingsSearchHit[] = [];
  for (const entry of entries) {
    const haystack = [entry.label, ...entry.keywords, entry.categoryId]
      .join(" ")
      .toLowerCase();
    const fullMatch = haystack.includes(q);
    const tokenMatch =
      !fullMatch && parts.length > 1 && parts.every((part) => haystack.includes(part));
    if (!fullMatch && !tokenMatch) continue;

    // Base score so token-only matches still rank above empty results.
    let score = tokenMatch ? 1 : 2;
    if (entry.label.toLowerCase().includes(q)) score += 10;
    if (entry.keywords.some((k) => k.toLowerCase().includes(q))) score += 5;
    if (entry.categoryId.includes(q.replace(/\s+/g, "-"))) score += 2;
    hits.push({ ...entry, score });
  }

  return hits.sort((a, b) => b.score - a.score || a.label.localeCompare(b.label));
}

const STORAGE_KEY = "coreside.settings.category";

export function readStoredSettingsCategory(): SettingsCategoryId {
  try {
    return normalizeSettingsCategoryId(
      typeof sessionStorage !== "undefined"
        ? sessionStorage.getItem(STORAGE_KEY)
        : null,
    );
  } catch {
    return DEFAULT_SETTINGS_CATEGORY;
  }
}

export function storeSettingsCategory(id: SettingsCategoryId): void {
  try {
    if (typeof sessionStorage !== "undefined") {
      sessionStorage.setItem(STORAGE_KEY, id);
    }
  } catch {
    // ignore quota / private mode
  }
}
