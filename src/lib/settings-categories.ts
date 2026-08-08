/**
 * Consumer Settings category model + search index.
 * Wallpapers live under Appearance. Added Settings are tool-created preferences.
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
  | "help-learning"
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
    description: "Theme and wallpapers.",
  },
  {
    id: "ai-access",
    label: "AI connections",
    group: "coreside",
    description: "How Coreside connects to AI on this computer.",
  },
  {
    id: "agent",
    label: "Assistant",
    group: "coreside",
    description: "How the assistant shows progress and activity detail.",
  },
  {
    id: "search",
    label: "Web research",
    group: "coreside",
    description: "Web discovery, page inspection, budgets, and research cache.",
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
    description: "Local chats, apps, and clear controls.",
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
    id: "help-learning",
    label: "Help & learning",
    group: "coreside",
    description: "Tours, shortcuts, and learning resources.",
  },
  {
    id: "about",
    label: "About",
    group: "coreside",
    description: "Coreside version and identity.",
  },
  {
    id: "added",
    label: "App settings",
    group: "added",
    description: "Preferences created by your apps.",
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
  // Legacy sessions that opened Added Settings only for wallpapers still resolve.
  if (value === "wallpaper" || value === "wallpapers") return "appearance";
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
  // dock-icon search entry intentionally omitted while manual Dock selection is
  // product-dormant (MANUAL_DOCK_ICON_SELECTION_ENABLED=false). Restore with the
  // ManualDockIconSelector mount when reactivating.
  {
    id: "wallpaper-link",
    categoryId: "appearance",
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
    label: "Activity",
    keywords: ["action log", "agent", "assistant", "progress", "steps"],
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
    keywords: ["clear", "storage", "conversations", "apps", "tools", "export", "backup", "recovery"],
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
    id: "help-learning",
    categoryId: "help-learning",
    label: "Help & learning",
    keywords: ["tutorial", "tour", "help", "shortcuts", "learning", "onboarding"],
  },
  {
    id: "version",
    categoryId: "about",
    label: "Version",
    keywords: ["about", "coreside"],
  },
  {
    id: "app-settings",
    categoryId: "added",
    label: "App settings",
    keywords: ["added settings", "tool settings", "app preferences"],
  },
];

export type SettingsSearchHit = SettingsSearchEntry & { score: number };

/** Stable DOM id for scrolling/focus after selecting a search hit. */
export function settingsTargetDomId(entryId: string): string {
  return `settings-target-${entryId}`;
}

export function matchSettingsSearch(
  query: string,
  entries: readonly SettingsSearchEntry[] = SETTINGS_SEARCH_INDEX,
  categories: readonly SettingsCategory[] = SETTINGS_CATEGORIES,
): SettingsSearchHit[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];

  const parts = q.split(/\s+/).filter(Boolean);
  const hits: SettingsSearchHit[] = [];
  const seen = new Set<string>();

  for (const entry of entries) {
    const category = categories.find((c) => c.id === entry.categoryId);
    const haystack = [
      entry.label,
      ...entry.keywords,
      entry.categoryId,
      category?.label ?? "",
      category?.description ?? "",
    ]
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
    if (category?.label.toLowerCase().includes(q)) score += 4;
    if (category?.description.toLowerCase().includes(q)) score += 3;
    if (entry.categoryId.includes(q.replace(/\s+/g, "-"))) score += 2;
    hits.push({ ...entry, score });
    seen.add(entry.id);
  }

  // Category name/description-only matches (no duplicate setting rows).
  for (const category of categories) {
    const haystack = [category.label, category.description, category.id]
      .join(" ")
      .toLowerCase();
    const fullMatch = haystack.includes(q);
    const tokenMatch =
      !fullMatch && parts.length > 1 && parts.every((part) => haystack.includes(part));
    if (!fullMatch && !tokenMatch) continue;
    const syntheticId = `category-${category.id}`;
    if (seen.has(syntheticId)) continue;
    if (hits.some((h) => h.categoryId === category.id)) continue;
    let score = tokenMatch ? 1 : 2;
    if (category.label.toLowerCase().includes(q)) score += 8;
    if (category.description.toLowerCase().includes(q)) score += 4;
    hits.push({
      id: syntheticId,
      categoryId: category.id,
      label: category.label,
      keywords: [],
      score,
    });
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
    if (typeof window !== "undefined") {
      window.dispatchEvent(
        new CustomEvent("coreside:settings-category", { detail: id }),
      );
    }
  } catch {
    // ignore quota / private mode
  }
}
