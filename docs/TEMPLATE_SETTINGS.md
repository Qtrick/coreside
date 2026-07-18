# Template Settings (Coreside)

**Template Settings** are an Added Settings subtype: extensible collections that start with built-in templates and allow user/agent-created entries without mutating Base Settings structure.

## Location

**Added Settings → Templates**

First category: **Wallpapers**

Future categories (not required in this phase): tool themes, layout presets, clock styles, dashboards.

## Rules

- Base Settings keep protected infrastructure (Appearance, Dock icon, AI Providers, Web Search and Research, Agent Behavior, Data, Accessibility, About).
- Wallpapers are **not** Base Settings.
- Built-in templates can be applied/previewed/duplicated; permanent deletion is blocked (hide/restore instead when implemented).
- User-created templates can be deleted with dependency checks.
- Renderers, motion limits, and readability remain protected; no arbitrary CSS/JS injection.

See [WALLPAPERS.md](./WALLPAPERS.md) and [READABILITY.md](./READABILITY.md).
