# Automations

Automations are local recurring routines. They run **only while Coreside is open**.

## Schedules

- Interval (minimum 5 minutes deterministic, 30 minutes AI-backed)
- Daily at a local time
- Weekly on a weekday at a local time

## Actions

Prefer deterministic actions:

- Cycle or set workspace background presets
- Set a safe tool state value

AI-backed automations must set `requiresAi`, warn about API credits, and fail clearly without a provider.

## Missed runs

On reopen, overdue automations catch up **at most once** (`run_once`) or skip (`skip`). No catch-up storms.

## Safety

- No shell commands, arbitrary JavaScript, or unprotected filesystem writes
- Concurrent duplicate runs are blocked
- Five consecutive failures pauses the automation
- Scheduler implementation and limits are protected (`core.automations.scheduler`)
