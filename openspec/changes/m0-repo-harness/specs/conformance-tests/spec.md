# Spec Delta

## Purpose

Defines the format of `.qnc` conformance fixtures, how the runner finds and judges them, and the coverage report that shows which spec rules have passing tests.

## ADDED Requirements

### Requirement: Fixture discovery
The runner SHALL treat every `*.qnc` file under `tests/conformance/testdata/` as one fixture and run fixtures in sorted path order. A `.stdout` file with no `.qnc` file of the same name SHALL be an error.

#### Scenario: Nested fixture
- **WHEN** `tests/conformance/testdata/bindings/shadow_same_scope.qnc` exists
- **THEN** the runner runs it as a fixture named `bindings/shadow_same_scope`

#### Scenario: Orphan output file
- **WHEN** `hello.stdout` exists and `hello.qnc` doesn't
- **THEN** the run fails and names `hello.stdout`

### Requirement: Fixture header
A fixture's header SHALL be its leading run of `//` comment lines and blank lines. It ends at the first line that is neither.

A header line of the form `// <word>:`, where `<word>` is one run of lowercase ASCII letters and hyphens, SHALL be a directive. The directives are `rule`, `mode` and `exit`, and any other lowercase word SHALL be an error. A case variant of a directive, such as `// Mode:`, SHALL be an error too. Other header comments, including `// Note: ...`, are free text.

#### Scenario: Blank line inside the header
- **WHEN** a fixture starts `// rule: bind.let`, a blank line, then `// mode: run`
- **THEN** the fixture is a run fixture citing `bind.let`

#### Scenario: Misspelled directive
- **WHEN** a header line is `// Mode: run` or `// rules: bind.let`
- **THEN** the fixture fails and names the unknown directive

#### Scenario: Free-text note
- **WHEN** a header line is `// Note: shadowing is rejected`
- **THEN** it is a free-text comment

### Requirement: Directives appear once, in the header
After the header, a comment line whose directive word is `rule`, `mode` or `exit` in any letter case SHALL be an error. A fixture SHALL NOT repeat `mode` or `exit`, and SHALL NOT cite the same rule twice.

#### Scenario: Directive after code
- **WHEN** a `// rule: c.d` line follows the first line of code
- **THEN** the fixture fails with a message to move the directive into the header

#### Scenario: Repeated mode
- **WHEN** a header sets `// mode: check` and then `// mode: run`
- **THEN** the fixture fails and names the repeated directive

### Requirement: Fixtures cite rules
A fixture SHALL cite one or more rules with `// rule: <id>` in its header. Each cited ID SHALL exist in the spec's rule set and SHALL NOT be retired.

#### Scenario: Missing citation
- **WHEN** a fixture has no `// rule:` line
- **THEN** the fixture fails with a message that a rule citation is required

#### Scenario: Unknown rule
- **WHEN** a fixture cites `bind.nonexistent`
- **THEN** the fixture fails and names the unknown ID

#### Scenario: Retired rule
- **WHEN** a fixture cites an ID listed in `spec/retired-rules.txt`
- **THEN** the fixture fails and says the ID is retired

### Requirement: Fixture modes
A fixture header MAY set `// mode: <mode>`, and the default is `check`. An unknown mode SHALL be an error. The modes are an open set, and later changes add modes (`fmt` in M1; test, query and bind modes later).
- `check` runs the whole checker as far as it exists (see Check fixtures).
- `parse` runs only the parser (see Parse fixtures).
- `run` builds and runs the file as a program (see Run fixtures).

`//~` expectations SHALL apply only to `check` and `parse` fixtures.

#### Scenario: Unknown mode
- **WHEN** a header sets `// mode: compile`
- **THEN** the fixture fails and lists the known modes

### Requirement: Check fixtures
A `check` fixture SHALL be a valid program under the whole spec apart from its expected diagnostics, so it keeps passing as later stages arrive. M1 wires only the `parse` backend, and the `check` backend arrives with the checker.

#### Scenario: Name-resolution fixture before the type checker
- **WHEN** an M2 check fixture tests an unknown import and also adds a `String` to an `Int`
- **THEN** the fixture is invalid as written, because it starts failing once M3's type checker reports the addition

### Requirement: Parse fixtures
A `parse` fixture SHALL be judged only on the parser's diagnostics, even after later stages exist, so a syntax fixture keeps passing when name resolution or lints add diagnostics.

#### Scenario: Later stage adds a warning
- **WHEN** a parse fixture declares `let x = 1` and never reads `x`
- **THEN** no unused-binding warning is reported for it, because only the parser runs

### Requirement: Run fixtures
A `run` fixture's program SHALL build without errors. Warnings aren't judged in `run` mode, so test them with a check fixture. `// exit: <n>` sets the expected exit code (default 0). Standard output SHALL equal the sibling `<name>.stdout` file, and a missing `.stdout` file means the program must print nothing. Standard error isn't compared in M0. A later change adds `<name>.stderr` when panic rules get tests.

#### Scenario: Run fixture with expected output
- **WHEN** `hello.qnc` has `// mode: run` and `hello.stdout` holds `hi\n`
- **THEN** the fixture passes only if the program exits with 0 and prints exactly `hi\n`

#### Scenario: Run fixture that doesn't build
- **WHEN** a run fixture's program has a type error
- **THEN** the fixture fails and lists the build's error diagnostics

#### Scenario: Run fixture without an output file
- **WHEN** a run fixture has no `.stdout` file and its program prints `hi\n`
- **THEN** the fixture fails

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

### Requirement: Expectation counting
One `//~` comment MAY list several codes separated by spaces. Each code SHALL expect one diagnostic, so `//~ E0001 E0001` expects two, and each diagnostic SHALL satisfy at most one expectation. A diagnostic that starts at the end of input after a trailing newline SHALL be matched against expectations on the last line.

#### Scenario: Two diagnostics on one line
- **WHEN** line 4 carries `//~ E0001 E0001` and the backend reports E0001 once on line 4
- **THEN** the fixture fails with one unmatched expectation

#### Scenario: Error at end of input
- **WHEN** a 5-line file ends with a newline, line 5 carries `//~ E0002`, and the backend reports E0002 at `[6, 1]`
- **THEN** the expectation is matched

### Requirement: No backend fails loudly
When no compiler backend is available for a fixture's mode, the fixture SHALL fail with a message saying which backend is missing. It SHALL NOT be skipped or counted as passing.

#### Scenario: Fixture before the parser exists
- **WHEN** a check-mode fixture is added and no check backend is wired up
- **THEN** the fixture fails with "no backend" and the run exits non-zero

### Requirement: Coverage report
Each run SHALL write `target/conformance/coverage.json` and print a per-section summary of total and covered counts. A rule is covered only when at least one passing fixture cites it.

#### Scenario: Empty suite
- **WHEN** the runner runs with no fixtures
- **THEN** the run succeeds and the report lists every rule ID as uncovered

#### Scenario: Failing fixture does not cover
- **WHEN** the only fixture citing `bind.let` fails
- **THEN** `bind.let` is reported as uncovered

### Requirement: Coverage report shape
The report SHALL have the shape `{ "spec_version", "total", "covered", "sections": [{ "section", "total", "covered", "rules": [{ "id", "covered_by": [<fixture name>] }] }] }`:
- `spec_version` is the spec's draft version, such as `"0.5"`, read from its header;
- sections appear in spec order, and each is named by its `## ` heading;
- grammar IDs come last, under the section `Grammar productions`;
- an empty `covered_by` means the rule is uncovered.

#### Scenario: Production ID in coverage
- **WHEN** the coverage report is generated
- **THEN** it lists `grammar.fn_decl` under `Grammar productions`
