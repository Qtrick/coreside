# Motion System (RC3.5)

**Product:** Coreside  
**Status:** Scaffolded / partially integrated

## Tokens (`src/styles/tokens.css`)

| Token | Role |
| --- | --- |
| `--motion-duration-fast` | Immediate feedback |
| `--motion-duration-enter` / `--motion-duration-exit` | Standard presence |
| `--motion-duration-panel` | Settings / Help / App panel |
| `--motion-duration-emphasized` | Rare emphasis |
| `--motion-ease-*` | Standard / enter / exit / emphasized |
| `--motion-distance` | Spatial offset budget |
| `--motion-duration-reduced` | Near-instant under reduced motion |

## Presence

`src/lib/motion/usePresence.ts` keeps components mounted until exit completes and cancels stale exits on rapid reopen. Under `prefers-reduced-motion`, spatial motion is skipped.

## Integration backlog

Welcome, tutorial — wired. Settings category body (including Help & learning) — wired with `usePresence` + panel motion tokens. App panel, Queue, wallpaper preview, progressive preview — wire presence + tokens without delaying required actions.
