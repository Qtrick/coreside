# Mandatory Session Close Rule: Suggested Commit Message

ALWAYS, after every work session, the agent (Codex, Cursor, Antigravity, or any other AI assistant) MUST provide a suggested commit message following Conventional Commits standards.

## Standards
1. **Type Prefix**: Use standard prefixes:
   - `feat:` for new capabilities or surface additions
   - `fix:` for defect fixes or regressions
   - `refactor:` for internal restructuring without behavior change
   - `test:` for adding or updating test suites and fixtures
   - `docs:` for documentation, guides, or specifications
   - `chore:` for build tooling, dependencies, auditor baselines, or asset generation
   - `perf:` for performance optimizations
2. **Scope (Optional)**: e.g. `feat(dock):`, `fix(migration):`, `chore(branding):`
3. **Subject**: Imperative mood, lowercase, concise (50-72 chars max), no trailing period.
4. **Body**: Detailed breakdown explaining *what* changed and *why*, referencing specific files, components, models, and evidence/testing outcomes.
5. **Footer**: Any breaking change notes, issue references, or release milestone tags.
