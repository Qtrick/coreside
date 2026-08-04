# Onboarding architecture

**Product:** Coreside  
**Slice:** RC3.4 vertical — first-run welcome + essentials tour

## Layers

1. **SQLite** `tutorial_progress` (migration `017`) — one row per `tutorial_id`.
2. **Rust** `db/tutorial.rs` — list/get/upsert/reset, eligibility, sample seed/cleanup.
3. **IPC** `get_onboarding_state`, `upsert_tutorial_progress`, `reset_tutorial_progress`, `seed_tutorial_sample`, `cleanup_tutorial_sample`.
4. **Frontend** definitions in `src/lib/onboarding/`, coordinator state machine, `useOnboardingStore`, `WelcomeDialog` + `TutorialOverlay` in `AppShell`.

## Eligibility (welcome)

Welcome shows when all hold:

- Main window (not `tool-*` secondary)
- Essentials status is absent or `not_started` (not `in_progress`, `completed`, `skipped`, or `superseded`)
- No meaningful prior activity (only tutorial sample ids allowed)
- Not recovery mode
- `CORESIDE_DISABLE_ONBOARDING` unset / not `1`, and not `CORESIDE_E2E=1`

`in_progress` resumes from **Help & learning → Continue unfinished**, not Welcome (so “Explore on my own” cannot skip a mid-tour).

## Persistence

Progress upserts go through Tauri → SQLite. Tours work offline without a provider.

Contextual tips and What’s new dismissals use the same `tutorial_progress` table with ids such as `contextual-first-app` and `whats-new:rc3.4`.

## Contextual first-use tips

`ContextualEducationHost` (AppShell) offers one nonmodal hint at a time for:

- First generated app (`contextual-first-app`)
- First approval (`contextual-first-approval`)
- First queued message (`contextual-first-queue`)
- First project create/open (`contextual-first-project`)
- First History / versions UI open (`contextual-first-versions`)

Tips never stack with Welcome or the essentials tour. Skipped when `onboardingDisabled` (includes `CORESIDE_DISABLE_ONBOARDING` and `CORESIDE_E2E`).

**Backup:** first backup action education is intentionally skipped — Settings → Data has “Back up now”, but there is no dedicated first-action education hook yet.

## What’s new

Static RC3.4 copy lives in `src/lib/onboarding/whats-new.ts`, shown under Help & learning. An optional dismissible banner appears for upgraded profiles with meaningful activity after essentials completed/skipped.

## Samples

`seed_tutorial_sample` creates fixed ids `conv-tutorial-sample` and `tool-tutorial-sample-planner` (idempotent). Cleanup deletes only those rows.

## Non-claims

This slice is Unit Verified at most. Do not claim Desktop Verified from this work alone.
