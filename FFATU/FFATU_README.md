# FFATU — Files for Agents to Use

**FFATU stands for “Files for Agents to Use.”**

FFATU is a private, local-only file-ingestion and reference workspace for AI agents.

Its primary purpose is to let users provide agents with files that cannot conveniently or directly be uploaded through the agent’s chat interface.

This is especially useful for agent environments such as Antigravity that may not support uploading certain file types, including:

- `.zip` archives
- Complete source-code repositories
- Large project directories
- Previous versions of a project
- Reference applications
- Screenshots and images
- PDFs and documents
- Reports
- Design assets
- Configuration examples
- Test fixtures
- Database files
- Logs
- Other supporting materials

Instead of uploading these materials through chat, the user can place them inside FFATU so the agent can access and inspect them directly from the local project filesystem.

FFATU acts as a manual local attachment system for agents.

---

# 1. Naming Rules

The main FFATU folder will normally be named:

```text
FFATU
```

Capitalization does not matter.

All of the following names must be treated as equivalent:

```text
FFATU
ffatu
Ffatu
FfAtU
```

For FFATU subfolders, capitalization differences also do not matter.

Spaces, hyphens, and underscores must be treated as equivalent separators.

For example, all of the following names refer to the same logical folder:

```text
Active-References
Active References
Active_References
active-references
ACTIVE_REFERENCES
```

Likewise, all of the following names refer to the same logical folder:

```text
Used-Files
Used Files
Used_Files
used-files
USED_FILES
```

Agents must not create duplicate folders merely because capitalization or separator style differs.

For example, if this folder already exists:

```text
active_references
```

the agent must not also create:

```text
Active-References
```

The preferred canonical names are:

```text
Inbox
Active-References
Used-Files
Temporary-Extractions
```

---

# 2. Primary Purpose

FFATU exists primarily to solve file-access and upload limitations.

The intended workflow is:

```text
The user places files into FFATU
        ↓
The agent discovers the files locally
        ↓
The agent inspects and analyzes them
        ↓
The agent uses them as reference for the assigned task
        ↓
The files remain active, are archived, or are removed as appropriate
```

FFATU may be used when:

- The chat interface does not support `.zip` uploads
- A full repository is too large to upload through chat
- The agent needs access to a complete directory structure
- Several related reference files need to be provided together
- The user wants the agent to compare the current project with an older version
- The user wants the agent to inspect a reference application
- The user wants screenshots, reports, and code archives available in one place
- The agent needs repeated access to the same reference material across multiple tasks

FFATU is not intended to replace Git, documentation, a package manager, a build system, or a secrets manager.

---

# 3. Example Use Cases

Examples of appropriate FFATU contents include:

- `Partial Update` as a reference for Coreside
- `Prebase-first-test` as a reference for Prebase
- An earlier archive of the current project
- A competing or related application used for comparison
- Screenshots showing a UI problem
- Screenshots showing a desired design
- A report from a previous development round
- A ZIP archive containing a reference repository
- A PDF containing requirements
- A folder containing design assets
- A database migration example
- A prior configuration file
- A set of test fixtures
- A document containing previous agent notes

These materials can help agents understand:

- What the project is intended to become
- What previously existed
- What is missing
- How a related system works
- What visual direction the user wants
- Which behavior should be reproduced
- Which behavior should be improved
- Which bugs or regressions need to be investigated

---

# 4. FFATU Is a Reference Workspace

FFATU materials may be used for:

- Context
- Analysis
- Comparison
- Inspiration
- Behavioral reference
- Architectural reference
- Historical reference
- Visual reference
- Feature-parity analysis
- Regression investigation
- Migration planning
- Test planning
- Security comparison
- Understanding user intent

Files inside FFATU are not automatically part of the active product.

Agents must not assume that a file in FFATU should be copied into the active project.

Agents must inspect, understand, and evaluate relevant materials before using them.

---

# 5. FFATU Must Not Become a Project Dependency

The active project must continue to function when FFATU is absent.

The active project must still be able to:

- Build
- Run
- Test
- Package
- Deploy
- Start from a fresh clone

without FFATU.

Agents must not:

- Import source files directly from FFATU
- Reference FFATU paths at runtime
- Make builds depend on FFATU
- Make tests depend on FFATU
- Make packaging depend on FFATU
- Make production assets depend on FFATU
- Require FFATU for deployment
- Require FFATU for normal application startup

If something from FFATU is genuinely needed in the active project, it must be intentionally:

1. Inspected
2. Evaluated
3. Sanitized
4. Adapted
5. Checked for licensing restrictions
6. Copied into the correct tracked project location
7. Tested as part of the active project

FFATU itself must remain optional and local.

---

# 6. The Active Project Remains the Source of Truth

The active project is the source of truth for:

- Current code
- Current architecture
- Current dependencies
- Current product requirements
- Current security requirements
- Current coding conventions
- Current tests
- Current build process
- Current packaging process
- Current deployment process
- Current supported platforms
- Current release status

FFATU provides supporting evidence and reference material.

It does not automatically override the active project.

A feature existing in an FFATU reference does not mean that the feature exists in the active project.

A feature working in a reference application does not mean that the current project has implemented or verified it.

Agents must compare reference materials against the current project before making conclusions.

---

# 7. Recommended Folder Structure

The preferred FFATU structure is:

```text
FFATU/
├── FFATU_README.md
├── FFATU_INDEX.md
├── Inbox/
├── Active-References/
├── Used-Files/
└── Temporary-Extractions/
```

A complete project example may look like:

```text
project-root/
├── .gitignore
├── AGENTS.md
├── docs/
├── src/
├── package.json
└── FFATU/
    ├── FFATU_README.md
    ├── FFATU_INDEX.md
    ├── Inbox/
    │   ├── newest-project-archive.zip
    │   ├── ui-problem.png
    │   └── development-notes.pdf
    ├── Active-References/
    │   ├── Partial-Update.zip
    │   ├── Prebase-first-test.zip
    │   └── design-reference-images/
    ├── Used-Files/
    │   └── previously-reviewed-report.pdf
    └── Temporary-Extractions/
        └── partial-update-extracted/
```

The exact capitalization or separator style may differ.

Agents must use any existing equivalent folder instead of creating duplicates.

---

# 8. `Inbox`

`Inbox` contains newly added materials that have not yet been reviewed or categorized.

Examples include:

- A newly added ZIP archive
- A new screenshot
- A newly provided PDF
- A new previous-project version
- A new design reference
- A newly downloaded repository
- A new report

An item should remain in `Inbox` until an agent determines:

- What it contains
- Whether it is relevant
- Which project area it relates to
- Whether it should become an ongoing reference
- Whether it should be used once and archived
- Whether it is irrelevant
- Whether it is unsafe to use

---

# 9. `Active-References`

`Active-References` contains materials that are expected to remain useful across ongoing project work.

Examples include:

- `Partial Update` during Coreside development
- `Prebase-first-test` during Prebase development
- A previous complete project version
- A continuing visual-design target
- A reference application used for feature-parity work
- A source archive containing several systems that will be analyzed over time

An item should remain in `Active-References` when:

- It will likely be needed again
- It applies to multiple future tasks
- Only some of its systems have been reviewed
- It remains a continuing comparison target
- It is useful for regressions
- It contains many subsystems
- It provides long-term design or architectural context

Agents must not move an important reference into `Used-Files` merely because one part of it was used once.

For example, reviewing the window-management system inside `Partial Update` does not mean the entire archive is finished if its settings, agent tools, onboarding, permissions, storage, or other systems may still be useful.

---

# 10. `Used-Files`

`Used-Files` contains materials that have been sufficiently processed for their intended purpose and are no longer part of the active working-reference set.

A file may be moved into `Used-Files` when:

- Its relevant contents were inspected
- Its intended task was completed
- Its useful findings were incorporated or recorded
- It is unlikely to be needed routinely
- It no longer needs to remain in the active queue

Moving a file into `Used-Files` does not necessarily mean:

- Every line was inspected
- Every system was implemented
- Every idea was accepted
- The source was secure
- The source was correct
- The source can never be used again

Agents may revisit `Used-Files` when:

- A regression appears
- A later task relates to the same material
- Previous analysis appears incomplete
- The user requests renewed analysis
- The reference becomes relevant again

A file may be moved back into `Active-References` if it becomes an ongoing reference again.

---

# 11. `Temporary-Extractions`

`Temporary-Extractions` contains temporary extracted copies and analysis-generated files.

Examples include:

- Extracted ZIP archives
- Extracted TAR archives
- Temporary repository copies
- Generated file inventories
- Comparison outputs
- Temporary conversion outputs
- Static-analysis reports
- Temporary screenshots
- Temporary notes created during inspection

Archives should normally be extracted into `Temporary-Extractions`.

Agents must not extract reference archives directly over the active project.

Temporary files should be deleted when they are no longer useful.

Original archives should normally remain unchanged.

---

# 12. Required Agent Workflow

When a task may depend on FFATU materials, agents should follow this process.

## Step 1: Inspect the Active Project

Before relying on a reference, inspect the current project.

Determine:

- Current architecture
- Existing implementation
- Existing tests
- Current dependencies
- Existing conventions
- Current security boundaries
- Known incomplete work
- Current build state
- Current packaging state

The active project must be understood before a reference is used to influence it.

---

## Step 2: Locate FFATU

Check whether a case-insensitive FFATU folder exists at or near the project root.

Examples include:

```text
FFATU
ffatu
Ffatu
```

If it exists, read:

```text
FFATU_README.md
```

before using its contents.

---

## Step 3: Locate Relevant Materials

Inspect:

- `Inbox`
- `Active-References`
- `Used-Files`
- `Temporary-Extractions`
- `FFATU_INDEX.md`
- Other files and folders inside FFATU

Remember that capitalization, spaces, hyphens, and underscores may vary.

Use existing equivalent folders rather than creating duplicates.

---

## Step 4: Inspect the Actual Files

Do not rely only on filenames.

Depending on the material, agents may need to:

- Extract archives
- Inspect directory structures
- Read source files
- Review package manifests
- Read configuration files
- Review tests
- Examine screenshots
- Read documents
- Inspect database migrations
- Inspect build configuration
- Compare current and previous versions
- Trace relevant execution paths
- Review assets
- Inspect logs and reports

When a complete repository or application is provided, agents should analyze it as a system rather than searching only for isolated snippets.

---

## Step 5: Compare Against the Active Project

Agents should distinguish clearly between:

- What exists in the active project
- What exists only in the reference
- What the user explicitly requested
- What the agent inferred
- What has been implemented
- What remains missing
- What has been verified
- What remains unverified

Useful questions include:

- What does the reference contain?
- What does the active project already contain?
- What is missing?
- What behaves differently?
- What should be adapted?
- What should be independently reimplemented?
- What should be rejected?
- What is outdated?
- What may be insecure?
- What does not fit the current architecture?

---

## Step 6: Use the Material Appropriately

A reference finding may be classified as:

- Reuse directly
- Adapt
- Reimplement
- Improve
- Use only as behavioral reference
- Use only as visual inspiration
- Reject
- Defer
- Research further
- Ask for human review

Agents must not blindly copy reference code.

The goal is to use the reference to understand the desired result and then implement the correct version for the current project.

---

## Step 7: Verify the Active Project

Any implementation influenced by FFATU must be verified in the active project.

Verification may include:

- Linting
- Type checking
- Unit tests
- Component tests
- Integration tests
- End-to-end tests
- Migration tests
- Security tests
- Build verification
- Desktop runtime testing
- Packaging verification
- Cross-platform testing
- Manual acceptance testing

Tests from the reference project do not count as verification for the active project.

---

## Step 8: Reorganize the Materials

After the task:

- Leave continuing references in `Active-References`
- Move suitable new materials from `Inbox` into `Active-References`
- Move sufficiently processed items into `Used-Files`
- Remove unnecessary temporary extraction files
- Update `FFATU_INDEX.md` when useful

Do not treat “used once” as equivalent to “fully processed.”

---

# 13. Optional `FFATU_INDEX.md`

For FFATU folders containing many files, agents may maintain:

```text
FFATU_INDEX.md
```

A recommended format is:

```markdown
# FFATU Index

| Item | Type | Purpose | Project Area | Status | Notes | Current Location |
|---|---|---|---|---|---|---|
| Partial-Update.zip | Source archive | Continuing Coreside reference | Desktop architecture and agent tools | Active Reference | Multiple systems remain relevant | Active-References/Partial-Update.zip |
| settings-bug.png | Image | UI bug reference | Settings | Not Reviewed | Pending inspection | Inbox/settings-bug.png |
| old-report.pdf | Document | Historical findings | Security | Used | Relevant findings already applied | Used-Files/old-report.pdf |
```

Suggested statuses include:

```text
Not Reviewed
Reviewing
Partially Reviewed
Active Reference
Partially Used
Used
Needs Reinspection
Blocked
Irrelevant
Unsafe
Duplicate
```

`FFATU_INDEX.md` remains inside FFATU and must not be committed.

The index is optional.

It is useful for organization, but FFATU’s primary purpose remains giving agents access to files they cannot receive through chat.

---

# 14. Context Preservation

FFATU preserves access to raw reference materials.

It does not automatically preserve everything a previous agent understood about those materials.

The core context flow is:

```text
Raw files remain available in FFATU
        ↓
A later agent can inspect them again
        ↓
The files continue to provide evidence and reference
```

For FFATU’s primary purpose, this is sufficient.

When an agent discovers an important architectural decision, security requirement, or long-term project conclusion, it may also record that conclusion in tracked project documentation.

Examples include:

```text
docs/reference-findings/
docs/architecture/
docs/decisions/
docs/security/
docs/parity/
```

This should be done when the conclusion is important enough that future agents or developers should know it without needing to reanalyze the original files.

It is not necessary to create tracked documentation every time an agent looks at a screenshot, ZIP archive, or temporary reference.

FFATU preserves the source material.

Tracked project documentation preserves important long-term conclusions.

---

# 15. Reference Material Is Not Automatically Production-Ready

Files inside FFATU may be:

- Outdated
- Incomplete
- Experimental
- Insecure
- Broken
- Deprecated
- Poorly tested
- Intended for a different framework
- Intended for another operating system
- Incompatible with the current architecture
- Licensed under different terms
- Included only for visual inspiration
- Included only for behavioral reference
- Generated by a previous agent
- Based on incorrect assumptions

Agents must independently evaluate relevant material.

Before adapting it, agents should consider:

- Security
- Correctness
- Maintainability
- Performance
- Compatibility
- Licensing
- Privacy
- Accessibility
- Dependency health
- Platform support
- Architectural fit
- User requirements

Reference materials should guide the implementation.

They should not automatically dictate it.

---

# 16. Security Rules

All FFATU materials must be treated as untrusted until inspected.

Agents must not:

- Execute unknown binaries
- Launch unknown applications automatically
- Run unknown scripts without inspection
- Run unknown installers without inspection
- Install unknown dependencies without reviewing their manifests
- Import unknown environment files into the active project
- Copy unknown configuration directly into production
- Expose credentials found inside reference files
- Upload FFATU files to external services without explicit permission
- Extract archives over the active repository
- Trust a file merely because it came from a previous project
- Assume reference code has been security-reviewed

Agents should inspect relevant files before executing or importing anything.

---

# 17. Archive Safety

Archives should normally be extracted into:

```text
Temporary-Extractions
```

or another clearly ignored temporary location.

Before extraction, agents should check for:

- Absolute paths
- Parent-directory traversal such as `../`
- Symlinks that escape the extraction directory
- Unexpected executable files
- Nested archives
- Extremely large expanded contents
- Private keys
- Credentials
- Environment files
- Machine-specific paths
- Dependency folders
- Generated build outputs
- Suspicious installation scripts
- Files that could overwrite the active project

Agents must not extract a reference archive directly over the active repository.

Original archives should normally remain unchanged.

---

# 18. Secrets and Private Information

FFATU may contain:

- API keys
- Access tokens
- Database credentials
- Environment files
- Private source code
- Proprietary documents
- Personal information
- Internal reports
- Certificates
- Private keys
- Machine-specific configuration

Agents must not reveal or propagate sensitive information.

If a credential or secret is discovered:

1. Do not print the full value.
2. Do not copy it into active source code.
3. Do not include it in tracked documentation.
4. Report that a sensitive value was found without reproducing it.
5. Recommend rotation when exposure may have occurred.
6. Replace it with a placeholder in examples.
7. Ensure the containing file remains ignored.

Safe placeholders include:

```text
YOUR_API_KEY
REDACTED_TOKEN
EXAMPLE_SECRET
PLACEHOLDER_CREDENTIAL
```

FFATU is not a secrets-management system.

Active credentials should use the project’s approved secure-storage mechanism.

---

# 19. Licensing

The presence of code or assets inside FFATU does not automatically grant permission to copy, modify, or redistribute them.

Agents should look for:

```text
LICENSE
COPYING
NOTICE
Copyright notices
Attribution requirements
Commercial restrictions
```

When licensing is unclear, agents should prefer:

- Behavioral analysis
- Architectural analysis
- Visual reference
- Compatibility analysis
- Independent reimplementation

rather than copying substantial source code directly.

---

# 20. Git Rules

**FFATU and everything inside it must never be committed to Git.**

This includes:

- `FFATU_README.md`
- `FFATU_INDEX.md`
- `Inbox`
- `Active-References`
- `Used-Files`
- `Temporary-Extractions`
- Archives
- Extracted files
- Screenshots
- Reports
- Logs
- Notes
- Reference repositories
- Temporary outputs
- Every other file under FFATU

The root `.gitignore` must contain:

```gitignore
# Local files and reference materials for AI agents — never commit
[Ff][Ff][Aa][Tt][Uu]/
```

This ignores capitalization variations such as:

```text
FFATU/
ffatu/
Ffatu/
fFaTu/
```

Because the entire FFATU folder is ignored, all subfolders and contents are also ignored.

---

# 21. Verify That FFATU Is Ignored

Agents must verify the ignore rule instead of assuming it works.

Recommended commands include:

```bash
git check-ignore -v FFATU/
git check-ignore -v FFATU/FFATU_README.md
git status --short
```

If the actual folder uses another capitalization:

```bash
git check-ignore -v ffatu/
```

To check whether any FFATU files are already tracked:

```bash
git ls-files | grep -i ffatu
```

This should return no tracked FFATU paths.

Agents must never force-add FFATU:

```bash
git add -f FFATU/
```

---

# 22. If FFATU Was Previously Tracked

Adding FFATU to `.gitignore` does not untrack files that Git already tracks.

If FFATU was previously committed, remove it from the Git index without deleting the local files:

```bash
git rm -r --cached FFATU
```

Use the actual folder capitalization when necessary.

Then verify:

```bash
git status --short
git ls-files | grep -i ffatu
```

If sensitive information was committed, removing it from the current index may not remove it from Git history.

Credentials exposed through Git history should be rotated.

Agents must not rewrite shared repository history without explicit user authorization.

---

# 23. Packaging and Deployment Exclusion

FFATU must not be included in:

- Production builds
- Application packages
- Desktop bundles
- Release archives
- Deployment uploads
- Docker images
- Docker build contexts
- CI artifacts
- Diagnostic bundles
- Crash reports
- Telemetry
- Source maps
- Installer contents
- Published packages
- Public documentation
- External backups intended for source distribution

Where relevant, agents should also exclude FFATU through files such as:

```text
.dockerignore
.npmignore
.tauriignore
```

The exact exclusions depend on the project’s build and deployment systems.

Any process that packages the entire repository root should be checked carefully.

---

# 24. Filename Collisions

Agents should preserve original filenames whenever practical.

If a file with the same name already exists, do not silently overwrite it.

Use a descriptive suffix such as:

```text
-original
-reviewed
-2026-08-06
-security-reference
-copy-2
```

Example:

```text
Used-Files/
├── Partial-Update-original.zip
├── Partial-Update-reviewed-2026-08-06.zip
└── Partial-Update-security-reference.zip
```

Agents must also avoid creating duplicate logical subfolders caused only by capitalization or separator differences.

---

# 25. Reporting Expectations

When FFATU materially affects a task, the agent should report:

- Which materials were inspected
- Where they were found
- What relevant systems or details were identified
- How they differ from the active project
- What was adapted
- What was independently implemented
- What was rejected
- What remains incomplete
- What was verified
- Which files remain active
- Which files were archived
- Which files remain unreviewed

Reports should use concrete paths and evidence when practical.

Avoid vague statements such as:

```text
The reference project was reviewed.
```

Prefer statements such as:

```text
Inspected the Partial Update archive in Active-References and compared its
window-management implementation with the current desktop bridge. The reference
behavior was used to understand dynamic resizing, but its unrestricted external
URL handling was not copied. The resizing behavior was independently
implemented behind the current project’s validated desktop command boundary.
```

---

# 26. Root `AGENTS.md` Pointer

Because FFATU is ignored by Git, a tracked root `AGENTS.md` should tell agents to check for it.

Recommended text:

```markdown
## Local Agent File Workspace

Before beginning substantial analysis or implementation, check whether a
case-insensitive `FFATU` folder exists in the project root.

`FFATU` means “Files for Agents to Use.” It is a local, untracked file-ingestion
and reference workspace for materials that may not be uploadable through the
agent’s chat interface, including ZIP archives, previous project versions,
reference repositories, screenshots, PDFs, reports, and design assets.

Folder capitalization does not matter. For FFATU subfolders, spaces, hyphens,
and underscores must be treated as equivalent separators.

Read `FFATU/FFATU_README.md` before using FFATU contents.

Never commit, package, deploy, upload, or expose FFATU or its contents. The
active project must continue to build, test, package, and run when FFATU is
absent.
```

This short pointer should be tracked.

The detailed `FFATU_README.md` remains local and ignored inside FFATU.

---

# 27. Non-Negotiable Rules

1. `FFATU` means **Files for Agents to Use**.

2. FFATU is primarily a local file-ingestion workspace for agent environments that cannot receive certain files directly through chat.

3. FFATU may contain archives, source repositories, previous project versions, images, documents, reports, and other reference materials.

4. Capitalization variations of FFATU and its subfolders must be treated as equivalent.

5. Spaces, hyphens, and underscores in FFATU subfolder names must be treated as equivalent separators.

6. The preferred ongoing-reference folder name is:

   ```text
   Active-References
   ```

7. Agents must not create duplicate logical folders because of naming variations.

8. Agents must read `FFATU_README.md` before using FFATU.

9. Agents should inspect the active project before relying on reference material.

10. Agents should inspect actual file contents rather than relying only on filenames.

11. FFATU material is reference material and is not automatically production-ready.

12. Agents must not blindly copy reference code.

13. FFATU must never become a runtime, build, test, packaging, or deployment dependency.

14. The active project must function when FFATU is absent.

15. Continuing references should remain in `Active-References`.

16. Files should move to `Used-Files` only when they are sufficiently processed for their intended purpose.

17. Using one part of a large reference does not mean the entire reference is finished.

18. Archives must be extracted into an ignored temporary location.

19. Archives must never be extracted over the active project.

20. FFATU materials must be treated as untrusted until inspected.

21. Unknown scripts, binaries, installers, and dependencies must not be executed without inspection.

22. Secrets found inside FFATU must not be exposed, copied, committed, or placed into tracked documentation.

23. External code and assets must be reviewed for licensing restrictions.

24. FFATU and every file inside it must never be committed.

25. The root `.gitignore` must contain:

   ```gitignore
   [Ff][Ff][Aa][Tt][Uu]/
   ```

26. Agents must verify that the ignore rule works.

27. FFATU must not be included in releases, deployments, packages, containers, CI artifacts, telemetry, logs, or diagnostic bundles.

28. A feature must not be considered complete merely because it exists in an FFATU reference.

29. Implementation and verification claims must be based on the active project.

30. FFATU preserves access to raw materials. Important long-term conclusions may be recorded separately in tracked project documentation when appropriate.