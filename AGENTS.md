# Coreside Codex Guide

Before substantive work, read `.cursor/rules/project-context.mdc`,
`.cursor/rules/Coreside-development.mdc`, and
`.cursor/rules/Ponytail-Code-Reduction.mdc`, plus the relevant architecture and
security documents in `docs/`. Preserve Coreside's secure typed-operation
architecture: Rust is the authority boundary and generated surfaces stay
declarative, validated, and capability-bounded.

Use inspect → plan → build → review → iterate. Prefer small, safe changes and
high-value tests. Research unstable APIs from primary sources. Record evidence
honestly by level and source identity; preserve historical evidence rather than
rewriting it. Never silently migrate user data or use unsafe Partial Update
mechanisms. Treat branch names case-insensitively. Use the test-writer and
code-reviewer-editor workflows. Ask before destructive operations.

## Code Review Rules

Review for correctness, security boundaries, race conditions, stale evidence,
persistence, rollback, capability enforcement, packaged behavior,
cross-platform regressions, and false readiness claims.
