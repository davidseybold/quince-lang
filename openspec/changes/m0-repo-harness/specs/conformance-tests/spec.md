# Spec Delta

## Purpose

Defines the format of `.qnc` conformance fixtures, how the runner finds and judges them, and the coverage report that shows which spec rules have passing tests.

## ADDED Requirements

### Requirement: Fixture discovery
The runner SHALL treat every `*.qnc` file under `tests/conformance/testdata/` as one fixture and run fixtures in sorted path order.

#### Scenario: Nested fixture
- **WHEN** `tests/conformance/testdata/bindings/shadow_same_scope.qnc` exists
- **THEN** the runner runs it as a fixture named `bindings/shadow_same_scope`

### Requirement: Fixtures cite rules
A fixture SHALL begin with one or more `// rule: <id>` header lines before any code. Each cited ID SHALL exist in the spec's rule set and SHALL NOT be retired.

#### Scenario: Missing citation
- **WHEN** a fixture has no `// rule:` line
- **THEN** the fixture fails with a message that a rule citation is required

#### Scenario: Unknown rule
- **WHEN** a fixture cites `bind.nonexistent`
- **THEN** the fixture fails and names the unknown ID

### Requirement: Fixture modes
A fixture header MAY set `// mode: check` (the default) or `// mode: run`. A `run` fixture MAY set `// exit: <n>` (default 0) and SHALL compare standard output against a sibling `<name>.stdout` file when one exists.

#### Scenario: Run fixture with expected output
- **WHEN** `hello.qnc` has `// mode: run` and `hello.stdout` holds `hi\n`
- **THEN** the fixture passes only if the program exits with 0 and prints exactly `hi\n`

### Requirement: Inline diagnostic expectations
`//~ CODE` SHALL expect a diagnostic with that code whose span starts on the same line. `//~^ CODE` SHALL expect one on the line above, and each further `^` moves one more line up. A fixture SHALL pass only if every expectation is matched and every error and warning diagnostic is expected.

#### Scenario: Expected error present
- **WHEN** line 4 carries `//~ E0412` and the backend reports E0412 starting on line 4
- **THEN** the fixture passes

#### Scenario: Unexpected diagnostic
- **WHEN** the backend reports W0003 on a line with no expectation
- **THEN** the fixture fails and shows the unexpected diagnostic

#### Scenario: Caret expectation
- **WHEN** line 6 carries `//~^^ E0100` and the backend reports E0100 on line 4
- **THEN** the expectation is matched

### Requirement: No backend fails loudly
When no compiler backend is available for a fixture's mode, the fixture SHALL fail with a message saying which backend is missing. It SHALL NOT be skipped or counted as passing.

#### Scenario: Fixture before the parser exists
- **WHEN** a check-mode fixture is added and no check backend is wired up
- **THEN** the fixture fails with "no backend" and the run exits non-zero

### Requirement: Coverage report
Each run SHALL write `target/conformance/coverage.json` listing every rule ID with its spec section and the passing fixtures that cite it, and SHALL print a per-section summary of total, covered and uncovered IDs. A rule is covered only when at least one passing fixture cites it.

#### Scenario: Empty suite
- **WHEN** the runner runs with no fixtures
- **THEN** the run succeeds and the report lists every rule ID as uncovered

#### Scenario: Failing fixture does not cover
- **WHEN** the only fixture citing `bind.let` fails
- **THEN** `bind.let` is reported as uncovered
