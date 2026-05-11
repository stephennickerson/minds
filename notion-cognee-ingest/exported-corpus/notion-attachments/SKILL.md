---
name: refactor
description: Apply Stephen's CODING_STANDARDS.md to an existing workspace. Use on triggers `/refactor`, "refactor this codebase", "apply the standards to [path]", "clean this up to standards", "bring this repo into compliance", or whenever Stephen wants code decomposed into seven-line singular-output functions with plain-English names, real-data verification, and synchronized Git closeout. Not for greenfield code generation. Not for single-file polish; use it when scope is at least a directory.
---

# Refactor

Apply Stephen's coding standards to an existing workspace. Survey. Plan. Execute. Verify. Synchronize.

## Standards Source

Read `references/CODING_STANDARDS.md` before planning or editing.

If the target workspace contains `CODING_STANDARDS.md` or `CODING_STANDARDS.MD`, read that too. Treat the workspace copy as the project-specific standard and the bundled copy as the reusable baseline.

If the two conflict, follow the stricter rule unless Stephen explicitly says otherwise.

## Triggers

- `/refactor`
- `/refactor <path>`
- "refactor this codebase against the standards"
- "apply the standards to [path]"
- "clean this up to standards"
- "bring this repo into compliance"

## Inputs

- **Scope**: default to the current working directory. Honor an optional path argument.
- **Standards**: read the bundled standards first, then any workspace standards file.
- **Languages**: detect from file extensions. Use the strongest available enforcement for each language.
- **Git**: follow the standards file unless Stephen explicitly suspends Git for the task.

## Phase 1: Establish Git Baseline

Before editing:

1. Check Git status.
2. Commit and push existing local changes when the repository has changes.
3. Pull the remote branch.
4. Begin the refactor only after the repository is synchronized.

If synchronization is not clean, fix synchronization first.

## Phase 2: Survey

Survey the requested scope and produce the working map:

- Every source file with line count and language
- Logical clusters by directory, module, or coherent slice of work
- Files excluded with reason
- Cluster count and rough work estimate

Exclude vendored, generated, third-party, lockfiles, build output, dependency folders, and binary assets unless Stephen explicitly includes them.

## Phase 3: Parallel Planning

For broad scopes, dispatch one planner sub-agent per independent cluster when sub-agents are available and the user explicitly permits parallel agent work.

Each planner must read the standards and walk every file against:

- Seven lines of logic per function
- One singular output per function
- One complete thought per function
- Plain-English names everywhere
- Comments only where ambiguity is genuinely unavoidable
- Exception handling only at real uncontrolled boundaries
- Strongest available language enforcement
- Real-data verification where behavior depends on real data

Each plan entry must include:

- File path
- Line range
- Current shape
- Target shape
- Proposed function or name changes
- Verification expected for the file

No code is written during planning.

## Phase 4: Consolidate

Merge plans into one execution manifest:

- Detect files touched by more than one plan
- Resolve naming conflicts
- Order shared helpers before dependents
- Keep file work separate so each file can be committed and pushed independently

The manifest is the input to execution.

## Phase 5: Execute

Walk the manifest one file at a time.

For each file:

1. Apply every planned change for that file.
2. Verify the file still follows the standards.
3. Run the strongest useful local check for that file or language slice.
4. Commit that file only.
5. Push the commit.
6. Move to the next file.

Do not batch edited files into one commit.

## Phase 6: Verify

Run the strongest available enforcement for every language touched:

- Rust: `cargo check`, `cargo clippy -- -D warnings`, and `cargo test` when tests exist
- TypeScript: `tsc --noEmit`, project linter, and project formatter
- JavaScript: project linter, project formatter, and `node --check` per file when applicable
- Python: configured type checker, `ruff` or project linter, and `pytest` when tests exist
- Other languages: compiler, static analyzer, linter, formatter, or native verification tool

When behavior depends on external data, verify with real data the user supplies, the repo provides, or an authorized live source provides.

Invented data is not final verification.

If real data is unavailable, state that plainly.

## Phase 7: Final Report

For every file touched, report:

- File path
- Commit hash
- Pushed branch
- Rule violations found
- Changes applied
- Verification result

For the workspace, report:

- Files surveyed
- Files in scope
- Files refactored
- Files already passing
- Files skipped with reason
- Outstanding violations the agent could not resolve

## Failure Conditions

The refactor is not complete if:

- Any refactored function exceeds seven lines of logic
- Any refactored function creates more than one singular output
- Any new name is abbreviated, type-prefixed, vague, or misleading
- A comment was added to compensate for unclear code
- Exception handling was added outside a real uncontrolled boundary
- Verification was claimed without running the strongest available enforcement
- Real-data behavior was claimed without real data
- Any edited file is uncommitted or unpushed
- The final report omits any edited file

## Review Checklist

Before stopping, confirm:

- Every refactored function creates one singular named output
- Every refactored function is seven lines of logic or fewer
- Every renamed thing reads as plain English
- Every comment is either unnecessary and removed or genuinely unavoidable
- Every exception boundary is real and external
- Every language's strongest available enforcement ran
- Every edited file has its own commit and push
- Every edited file is named in the final report with commit and verification

