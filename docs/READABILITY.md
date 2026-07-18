# Readability

Minimal contrast helpers and semantic surface tokens so UI text stays readable over live wallpapers.

## Semantic tokens

Defined in `src/styles/tokens.css`, mapped from theme vars:

- `--core-text-primary` / `--core-text-secondary`
- `--core-surface-primary` / `--core-surface-muted`
- `--core-background` / `--core-border` / `--core-accent`
- `--core-content-overlay` / `--core-sidebar-overlay` — stronger translucent surfaces when a wallpaper is active

When `.app-shell[data-wallpaper]` is set, sidebar/chat/settings panels use the overlay tokens plus light blur so foreground text keeps contrast without hiding the wallpaper.

## Contrast helpers

`src/lib/readability/contrast.ts`:

- `parseHexColor` — `#rgb` / `#rrggbb`
- `relativeLuminance` — WCAG 2.x relative luminance
- `contrastRatio` — WCAG contrast ratio (1–21)

Protected as product readability utilities (`core.readability.*`); agents must not replace these with arbitrary CSS injection.
