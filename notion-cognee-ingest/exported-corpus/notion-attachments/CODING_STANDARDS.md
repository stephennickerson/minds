# Coding Standards for AI Agents

These standards are binding instructions for AI agents.

The purpose of these standards is to prevent AI slop permanently: broken changes, regression, vague code, fake verification, forgotten files, and future work becoming expensive because the code was not decomposed into complete units from the beginning.

## Core Law

Every function creates one singular output.

The function name states that output in plain English. The body creates that output. If creating that output requires more than 7 lines of logic, decompose the body into smaller functions. Each smaller function must also create one singular output and obey the same rule.

This rule is recursive.

## Seven Lines of Logic

A function may contain at most 7 lines of logic.

The count excludes:

- Braces
- Blank lines
- Pure variable declarations
- Formatting required by the language

The count includes:

- Branches
- Loops
- Assignments that transform data
- Calls that produce part of the named output
- Return expressions
- Any line that changes, selects, derives, validates, loads, saves, sends, renders, mutates, or decides

If a function needs more than 7 lines of logic, the function is too large. Decompose it.

## One Complete Thought

A complete thought is one unit of work with one outcome.

A function may:

- Accept the ingredients needed to create its named output
- Create that output directly
- Call smaller singular-output functions to create that output
- Return the output

A function may not mix multiple outcomes into one body.

If the body first turns input into one output and then uses that output to create another output, the function contains two thoughts. Decompose it.

## Naming

Every name must read as plain English.

This applies to:

- Files
- Modules
- Functions
- Methods
- Variables
- Structs
- Enums
- Traits
- Types
- Tests
- Scripts
- Directories created for the work

Names must explain the code by naming the objects and outcomes clearly.

Do not abbreviate.

Do not encode the type into the name.

Use:

```text
get_phone_number(first_name, last_name)
```

Do not use:

```text
get_phone(str_first_nm, str_last_nm)
```

The name, inputs, intermediate names, and output must tell one obvious story.

## Documentation and Comments

The code should explain itself through names and decomposition.

Comments are not a substitute for clear naming or smaller functions.

If a comment appears necessary, the code is probably not decomposed or named correctly. Fix the code shape first.

Comments are allowed only for genuinely unavoidable ambiguity that cannot be removed by better naming or decomposition.

## Correctness

Prefer languages and tools where correctness is enforced automatically.

Use the strongest available enforcement mechanism in the language:

- Rust: compiler, type system, ownership, pattern matching, and impossible invalid states
- TypeScript or JavaScript: strict typing where available, linting, format checks, runtime contract checks only when unavoidable
- Other languages: compiler, static analysis, linter, formatter, and language-native guarantees

The standard is not that every language behaves like Rust. The standard is that the agent must use the strongest automatic enforcement available in the language being edited.

Invalid states should be made impossible by code shape, naming, decomposition, and language enforcement.

Exception handling is not the answer to bad design.

Do not add exception handling as a compensation for unclear code, weak decomposition, fake certainty, or preventable invalid state.

Only handle external uncertainty at the real boundary where uncontrolled data enters the system. Do not invent theoretical boundaries to justify defensive clutter.

## Testing and Verification

TDD is not required and should not be used as the primary proof of correctness.

Tests are not proof when they rely on invented data, mocked success, or synthetic scenarios that let the agent claim correctness without touching reality.

All final verification must use real data when real data is needed.

Real data means data the AI did not invent:

- Requested from the user
- Discovered in the repository
- Pulled from the internet
- Imported from a real export
- Loaded from a live or production-like source
- Captured from an actual external system

Temporary test data may be used as a workbench while building. It is not proof. It is not final verification. It must not be presented as evidence that production behavior is correct.

If real data is unavailable, the agent must say so plainly in the final report.

Do not claim work is tested unless the verification used real data or the claim clearly states the limitation.

## Git Baseline

Before starting edits, establish a synchronized baseline:

1. Check Git status.
2. Commit and push any existing local changes.
3. Pull the remote branch.
4. Begin the requested work.

The agent must leave the repository synchronized.

If Git is not synchronized, the current task is to fix synchronization first, then continue the work.

## Git Commit Rule

Every created or edited file must be committed and pushed.

One file equals one commit.

Do not batch multiple files into one commit.

Commit and push immediately after each file edit is complete.

At stop, every created or edited file must already be committed and pushed.

The one-file commit rule exists to prevent forgotten files.

## Commit Messages

Use this shape:

```text
<work item>: <created output>
```

Examples:

```text
profile import: create validated customer record
chat history: preserve message order
agent prompt: enforce seven-line logic rule
data sync: load real account records
ui state: prevent invalid selected conversation
```

When useful, append verification:

```text
profile import: create validated customer record; verified with exported Stripe data
```

## Final Report

At stop, report:

- Each created or edited file
- Commit hash for that file
- Pushed branch
- Real-data verification used
- Any file that could not be verified with real data

Do not hide missing verification.

Do not hide Git failures.

Do not claim completion while files remain uncommitted or unpushed.

## Agent Failure Conditions

The work is not complete if:

- Any function exceeds 7 lines of logic
- Any function creates more than one singular output
- Any name is abbreviated, type-prefixed, vague, or misleading
- A comment compensates for unclear code
- Exception handling is used to cover preventable design failure
- Verification relies on invented data while claiming real correctness
- A created or edited file is not committed and pushed
- The final report omits file, commit, branch, or verification evidence

## Review Checklist

Before stopping, inspect the work against this checklist:

- Does every function create one singular named output?
- Is every function 7 lines of logic or fewer?
- Does every name read as plain English?
- Does the code explain itself without documentation?
- Are invalid states prevented by code shape and automatic enforcement?
- Is exception handling absent unless it guards a real uncontrolled boundary?
- Was real data used for final verification when real data matters?
- Is every created or edited file committed and pushed in its own commit?
- Does the final report give the file, commit hash, branch, and verification evidence?

