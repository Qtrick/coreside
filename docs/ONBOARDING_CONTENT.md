# Onboarding content

## First-run module: `coreside-essentials` v1

| Step id | Kind | Target |
| --- | --- | --- |
| `welcome` | welcome | — |
| `chat-composer` | spotlight | `chat-composer` |
| `app-panel` | spotlight | `app-panel` |
| `preview-review` | spotlight | `chat-composer-send` |
| `help-learning` | completion | `help-learning-nav` |

CTA copy: “Take the 3-minute tour” / “Explore on my own”.

## Advanced (listed, not auto-shown)

- `coreside-projects` — sidebar projects
- `coreside-permissions` — approval boundaries
- `coreside-versions` — undo / history
- `coreside-shortcuts` — command palette (`developerOnly`)

## Contextual tips (auto, one-shot)

| Id | Trigger |
| --- | --- |
| `contextual-first-app` | `activeTool` set |
| `contextual-first-approval` | pending approval appears |
| `contextual-first-queue` | queue has items |
| `contextual-first-project` | project list/detail opened |
| `contextual-first-versions` | Conversation History opened |

Backup first-action tip: **skipped** (no clear education hook yet).

## What’s new

- Progress id: `whats-new:rc3.4`
- Help & learning section + optional upgrade banner

Edit copy in `src/lib/onboarding/tutorials.ts` / `whats-new.ts`. Keep step ids stable once shipped.
