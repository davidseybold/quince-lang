# Spec Delta

## Purpose

Provides one command, `cargo xtask ci`, that runs every fast check in the repo, so agents and CI agree on what green means.

## ADDED Requirements

### Requirement: One command runs every check
`cargo xtask ci` SHALL run formatting, lints with warnings denied, the workspace tests, the rule-ID check, the grammar check, the diagnostics check and the conformance suite.

#### Scenario: Clean tree
- **WHEN** `cargo xtask ci` runs on a tree where every check passes
- **THEN** it prints a table with every step passed and exits 0

### Requirement: All steps run and all failures are reported
`cargo xtask ci` SHALL run every step even after one fails, then print a pass/fail table and exit non-zero if any step failed.

#### Scenario: Two failing steps
- **WHEN** formatting and the grammar check both fail
- **THEN** both are reported as failed in the summary, the remaining steps still ran, and the exit code is non-zero

### Requirement: Each step runs alone
Each check SHALL also be available as its own subcommand (`cargo xtask spec-rules`, `spec-grammar`, `diagnostics`, `conformance`).

#### Scenario: Rerunning one step
- **WHEN** an agent runs `cargo xtask spec-grammar`
- **THEN** only the grammar check runs, with the same result it has inside `ci`

### Requirement: CI runs the same command
The GitHub Actions workflow SHALL run `cargo xtask ci` with the pinned toolchain on every pull request, whatever its base branch, and on every push to `main`. Its token SHALL be read-only.

#### Scenario: Pull request opened
- **WHEN** a pull request is opened, including one based on another branch of a stack
- **THEN** the workflow runs `cargo xtask ci` and reports its result as a check

### Requirement: Files keep their bytes
Git SHALL NOT convert line endings in this repo, so the strip and grammar checks compare the same bytes on every platform, and fixtures can contain CRLF on purpose.

#### Scenario: Checkout with autocrlf
- **WHEN** the repo is cloned with `core.autocrlf=true`
- **THEN** every file keeps the line endings it was committed with
