# Settings Information Architecture

**Product:** Coreside  
**Access date:** 2026-08-01  
**Status:** Proposed target IA (current UI is flatter; migrate deliberately)  
**Current UI:** `src/components/settings/SettingsPanel.tsx` + related panels

## Principles

1. **Consumer-first labels** — everyday language in primary nav and headings.
2. **Developer / protocol language** stays in Advanced, docs, or secondary hints—not first-run copy.
3. **Protected Base Settings** remain product-owned; **Added Settings** stay user/tool-owned (wallpapers under Templates).
4. One job per category; avoid dumping every toggle into Appearance.

## Proposed Base Settings categories

| Category | Consumer purpose | Examples (move or keep) | Avoid saying |
| --- | --- | --- | --- |
| **General** | Everyday app behavior | Startup chat, Adaptive window sizing (Smart / Ask first / Off), language later | “Orchestrator policy” |
| **Appearance** | Look of the window chrome | Theme, accents, borders, Dock icon | Wallpaper (lives in Added → Templates) |
| **AI Access** | How you connect to models | BYOK providers, test connection, key status (`keyDetected` only) | Raw keyring / IPC jargon |
| **Agent** | How the assistant behaves | Action Log Off / Always / Intelligent; future confirmativeness | “Tool loop max steps” in primary UI |
| **Search** | Web research budgets & safety | Search profile, monthly budget summary, safe search | Exa account internals as primary copy |
| **Privacy** | What leaves the device | Local-first notice, export redaction summary, Action Log privacy | Chain-of-thought / prompt dumps |
| **Data** | Local storage controls | Clear conversations / tools (confirm), storage location in plain words | SQL table names |
| **Accessibility** | Inclusive use | Reduced motion (system), future contrast/text size | “WCAG certified” |
| **Advanced** | Rare / power controls | Recovery Mode, Runtime Permissions, diagnostics entry points | Hiding Recovery here permanently—Recovery must stay findable |
| **About** | Identity | Coreside name, version, short product description | Provider co-branding as product name |

## Added Settings (unchanged ownership)

| Area | Content |
| --- | --- |
| **Templates → Wallpapers** | Presets, Live badge, Interface transparency, Clear/None |
| **Tool-owned settings** | Per-tool Added Settings rows |

Wallpaper remains **not** a Base Settings structural category (product rule).

## Current → proposed mapping

| Today (approx.) | Proposed home |
| --- | --- |
| Appearance (theme, dock, adaptive window) | Appearance + General (adaptive window) |
| AiProviderSettings | AI Access |
| AgentBehaviorSettings (Action Log) | Agent |
| Data clear actions | Data |
| Accessibility blurb | Accessibility |
| RecoverySettings / RuntimePermissionsSettings | Advanced (Recovery still labeled clearly) |
| About | About |
| Added → Templates → WallpaperSettings | Added Settings (keep) |

## Language guide

| Prefer | Avoid in consumer UI |
| --- | --- |
| “Connect your AI provider” | “Configure adapter credentials” |
| “Ask before changing” | “RequireApproval gate” |
| “Stored only on this computer” | “SQLite under app data dir” |
| “Interface transparency” | “Panel alpha compositing” |
| “Recovery” | “Kernel recovery flag” |

Technical terms may appear in Advanced descriptions and developer docs.

## Migration notes

- Reorganize navigation without breaking deep links / focus order.
- Do not move Wallpapers into Base Settings structure.
- Keep agent-editable allowlists unchanged when relocating UI chrome.
- Ship category labels incrementally; do not block wallpaper compositing on full IA rewrite.

## Related

- [TEMPLATE_SETTINGS.md](./TEMPLATE_SETTINGS.md)
- [WALLPAPERS.md](./WALLPAPERS.md)
- [SECURITY.md](./SECURITY.md)
