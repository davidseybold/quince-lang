# Agent-effectiveness benchmark plan

Status: plan, written 2026-11-13 against spec Draft v0.26. Nothing in it is built yet. When Phase 1 starts, the harness gets its own design doc in `docs/design/` (with its task list), and this file keeps only the plan and the results history.

## Why

Quince is designed to be the best possible language for AI coding agents ([CLAUDE.md](../CLAUDE.md), [design principles](design-principles.md)). That is a claim about outcomes, so it should be measured, not argued. This plan describes how to measure it once the toolchain runs programs: give agents the same tasks in Quince and in comparison languages, grade every attempt with the same hidden tests, and compare how often they succeed, what success costs, and how often wrong code gets past them.

The benchmark has two jobs, and they pull in different directions:

1. **A headline result** for 1.0: is an agent more effective in Quince than in Go, Rust, Python and TypeScript, once training-data familiarity is accounted for?
2. **A design feedback loop** during implementation: every failed Quince attempt is a pointer to a spec gap, an unclear diagnostic, a missing std function or a surface that invites the wrong guess. The spec agent tests (runs 1 to 7, counting meaning-changing guesses) were an early version of this, and the meaning-guess count stays a metric here.

The second job starts paying off long before the first is credible, so the rollout starts small and early.

## Hypotheses

Written before any run, so the results can't pick their own questions. Each full run (Phase 3 and 4) restates them with numbers before it starts.

- **H1, pass rate.** With the spec and std API available to read, agents pass at least as many tasks in Quince as in Go, and the gap to Python and TypeScript shrinks with each phase.
- **H2, bug escape.** Of the attempts that pass the visible tests, fewer Quince attempts fail hidden tests on error paths, cancellation and concurrency than in any comparison language. This is where effects, exhaustive `match`, `Result` and the checker should show.
- **H3, edit robustness.** Changing an existing Quince codebase breaks unrelated behavior less often than in the comparison languages, because a function's effects and failure modes are in its signature.
- **H4, cost to success.** A successful Quince attempt needs fewer compile cycles after the first failing compile, because diagnostics carry fixes and `quince query` answers questions an agent would otherwise answer by reading files.
- **H5, review.** A reviewing agent finds a planted bug in Quince code more often, and with fewer tokens, than in Go.

H1 is the one most likely to fail early, and that's acceptable: the models have never seen Quince. The controls below are what make H1 meaningful.

## Comparison languages

| Language | Why it's in |
| --- | --- |
| **Go** | The closest in target domain (services and tools, green threads, simple surface). The primary comparison from Phase 2 on. |
| **Rust** | The closest in surface and in checking strength. Tests whether Quince's checker helps without Rust's ownership cost. |
| **Python** | Where models are strongest. The ceiling for familiarity. |
| **TypeScript** (Node) | Very familiar, statically typed, and the other half of Quince's "familiar surface". |
| **Unseen baseline: Gleam** | A real, statically typed language with far less training data than the others, and with an HTTP stack, so it can attempt every category. It separates "Quince's design" from "any new language". |

Alternatives for the unseen baseline, and why they aren't first choice:

- **Koka** has effect types, which would make it the sharpest comparison for H2, but it lacks the HTTP and file libraries the service tasks need. It's worth adding for the CLI, concurrency and error-path categories only, if the budget allows.
- **Roc** and **Hare** are less mature than Gleam, so failures would measure the toolchain more than the language.
- **Zig** now has a lot of training data, and isn't "unseen" any more.

Gleam's share of training data will grow, so each run records the model and its training cutoff, and the report compares Quince to Gleam by run date, not across dates.

**Optional control, a disguised Go.** Go with its keywords and std names systematically renamed (`func` to `fun`, `fmt.Println` to `out.say`, and so on) and a one-page spec. It has Go's semantics but no training data, so the gap between Go and disguised Go measures pure surface familiarity. If Quince with its spec beats disguised Go with its spec, the design is doing work beyond familiar syntax. This costs a transpiler, so it waits until Phase 3 shows it's needed.

## The task suite

### What a task is

A task is written once, independent of language, and graded from outside the program. Each task lives in `bench/tasks/<id>/`:

| File | What it holds |
| --- | --- |
| `task.toml` | Id, category, the milestone it needs (so a phase can select tasks that the toolchain can run), the language features it exercises, and its budget overrides if any. |
| `prompt.md` | The task as the agent sees it. It never names a language construct or library; the harness adds one line naming the language. |
| `visible/` | Test cases the agent can run through `./check`. About a third of the cases, covering the main path. |
| `hidden/` | The rest: error paths, edge cases, concurrency limits, cancellation, malformed input. Never copied into the agent's sandbox. |
| `reference/<lang>/` | A solution in every benchmark language that passes every hidden case. It proves the task is solvable in each language and that the harness grades correctly. |
| `starter/<lang>/` | For edit, bug-fix and review tasks only: the existing codebase, written to the same design in each language. |
| `regression/` | For edit tasks only: cases for the behavior the change must not break. |

Starters are the hardest part to make fair. Each language's starter has the same modules, the same names (in each language's naming case) and the same size within about 20%, and is written in that language's ordinary style, not translated line by line. A reviewer fluent in the language checks each one.

### Categories

| Category | Target share | What it tests | Example tasks |
| --- | --- | --- | --- |
| **CLI tools** | 20% | Args, files, stdin and stdout, exit codes, error messages | `wc` with per-file errors and a total; CSV to JSON Lines with quoted fields and line-numbered errors; an access-log summarizer with top-N paths and a status histogram; find duplicate files in a tree by content hash, with a deterministic order |
| **HTTP services** | 20% | Routing, JSON encoding and decoding, validation, state, persistence | A key-value store with TTLs; a URL shortener that persists to a file across restarts; a todo API whose 400 responses name each invalid field; a per-client token-bucket rate limiter in front of an upstream |
| **Concurrency and cancellation** | 15% | Fan-out limits, timeouts, cancellation, ordering, channels | Fetch N URLs with at most K in flight and a per-request timeout, output in input order; query three mirrors and return the first success, cancelling the rest; a bounded pipeline with a parallel stage and ordered output; retries with backoff under a total deadline |
| **Error handling and data** | 10% | Precise errors, all-or-nothing updates, serialization round trips | A strict config parser that reports every error with line and column; a ledger that rejects overdrafts and applies each batch atomically; a JSON schema subset validator |
| **Feature edits** | 15% | Changing a 1,000 to 3,000 line codebase without breaking it | Add pagination and filtering to an existing todo service; add a `--json` output mode to an existing CLI; extract storage behind an interface and add a file-backed implementation |
| **Bug fixes** | 10% | Locating and fixing a seeded bug from a failing report | An off-by-one in a top-N tie-break; a task leak when a fan-out is cancelled; an error swallowed and turned into a default value |
| **Review** | 5% | Reading for agents (H5) | Find the planted bug in a 500-line diff; say whether a change alters a public API's behavior |
| **C interop** | 5% | Wrapping a C library (Quince, Go with cgo, Rust, Python ctypes) | Wrap a small compression library; added in Phase 3, after M12 |

The full suite is about 150 tasks. Categories, not individual tasks, are the unit the report breaks results down by, so each needs at least 10 tasks in the full suite to say anything on its own.

### What makes a good task

- **Solvable in about 50 to 400 lines** in Go, so that a run fits the budget and failures come from the language, not from running out of turns.
- **Graded only on behavior.** No task checks style, structure or performance beyond a generous time limit. A task that can only be graded by reading the code doesn't go in (review tasks are graded on the agent's answer, which is a structured verdict, not code).
- **Hidden cases that a careful agent would think of** from the prompt alone: if the prompt says "a missing file is an error", a hidden case tests a missing file. Hidden cases never test behavior the prompt doesn't state. The prompt may be terse, but it is complete.
- **No dependency on a library outside the allowed set** (see [Fairness](#fairness)).
- **Not built to favor Quince.** Concurrency and error paths are where Quince should shine, and the category shares above are what services and tools look like in practice, not what flatters the design. The shares are fixed before Phase 3 and not changed after seeing results.

## The harness

### Grading programs from outside

Every case is a black-box check against the built program, run by a harness written once in Rust (`bench/harness`, crate `quince_bench`, no LLVM). Cases are declarative TOML where possible, so a task author never writes harness code:

- **Process cases:** arguments, stdin, files placed in a temporary working directory, environment variables; expected stdout (exact, as JSON compared structurally, or by regex), stderr by regex, exit code, and the files left behind.
- **HTTP cases:** the harness starts the program with `PORT` set, waits for the port, then sends a sequence of requests with expected statuses, headers and JSON bodies. A case can restart the program between requests to test persistence.
- **Fake upstreams:** the harness runs scriptable HTTP servers the program talks to. A fake can delay, fail, hang, return garbage, and record what it saw: the number of requests in flight at once, the requests that were cancelled (connection closed before the response), and the order of arrival. This is what grades concurrency limits and cancellation from outside, without trusting the program's own output.
- **Timing:** no case depends on a tight wall-clock margin. A TTL case uses a TTL of 1 second and waits 2.5, a timeout case uses a fake that hangs forever. Every case is run 3 times against every reference solution in CI before a task is accepted, to catch flaky cases.

The same case files back `./check` (visible cases only) inside the agent's sandbox and the grader (all cases) outside it, so the agent's feedback and the grade can't drift apart.

### Running agents

- **One agent scaffold for every language.** A minimal loop with three tools: run a shell command, read a file, write or edit a file. The system prompt is identical except for the language name, the toolchain's commands, and where the docs are. A minimal scaffold avoids measuring how well a product's prompts were tuned for popular languages. A second configuration, Claude Code run headless with default settings, is added in Phase 3 as the realistic case.
- **The sandbox** is a container with the language's pinned toolchain, the allowed libraries already installed, the task's prompt and visible cases, and no network beyond the harness's fake upstreams.
- **The budget** is the same for every language: initially 60 turns, 1.5 million tokens and 30 minutes of wall-clock time, whichever comes first. The agent stops when it says it's done or the budget runs out, and the final state of the sandbox is graded either way.
- **Models:** at least two in each full run, a frontier model and a small, fast one. Small models lean harder on familiarity, so the gap between the two is itself a result. Every result row records the exact model ID.

### Context conditions

The training-data confound is controlled by varying what's in context, for every language:

| Condition | What the agent gets |
| --- | --- |
| **C0, nothing** | The prompt and the toolchain. For the comparison languages this is how agents are used today. For Quince it measures how far the familiar surface carries an agent alone. |
| **C1, docs** | C0 plus the language's reference and standard library docs as local files the agent can read on demand (for Quince, `spec/` and `std/API.md`; for the others, their official reference and std docs, pinned to the toolchain's version). Not pasted into the prompt, so a long spec doesn't cost tokens it isn't asked for. |
| **C2, docs and a guide** | C1 plus a short guide of at most 4,000 tokens: idioms, the commands, and common mistakes. Each language's guide is written to the same template, and Quince's is not iterated against the benchmark's own tasks (that would be training on the test set). |

The main comparison is Quince at C1 against each comparison language at its best condition, which is usually C0 or C1. If Quince at C1 matches Go at C0, the spec is doing the work that training data does for Go, which is the strongest result this benchmark can give. The C0 to C1 to C2 curve for Quince also shows how much each kind of documentation is worth, which guides what to write.

### Tooling ablations

Quince's claim is partly about its tools, so Phase 3 adds Quince-only conditions that turn one off at a time:

- JSON diagnostics with fixes, against the same diagnostics as plain text without fixes.
- `quince query` available, against not installed.
- `quince fix` available, against not installed.

Each ablation that makes no measurable difference is a sign the tool isn't pulling its weight for agents, or that agents don't discover it, and the system prompt doesn't mention any tool beyond `quince build`, `check` and `test`, so discovery is part of what's measured.

## Metrics

Every run writes one JSON line with the task, language, condition, model, seed, the result of every case, token counts, turn count, tool calls, compile and test cycles, wall-clock time, and the path of its transcript. Every metric below is computed from those lines, so a new metric can be computed over old runs.

| Metric | Definition | Tests |
| --- | --- | --- |
| **pass@1** | Fraction of runs whose final state passes every hidden and visible case. The headline number. | H1 |
| **pass@k** | Chance that at least one of k runs passes, with the unbiased estimator `1 - C(n-c, k) / C(n, k)` over n runs with c passes. Reported for k = 1, 3, 5. | H1 |
| **Cost to success** | Tokens, turns, compile cycles and wall-clock time, for passing runs only, plus **expected cost per solved task**: total cost over all runs divided by the number that passed. The second can't hide a low pass rate. | H4 |
| **Compile cycles after the first error** | The number of compiles from the first failed compile to the first clean one. Measures how well diagnostics lead to a fix, in every language. | H4 |
| **Bug escape rate** | Of the runs that pass every visible case, the fraction that fail at least one hidden case. Broken down by the hidden case's tag (error path, cancellation, concurrency, edge case). | H2 |
| **Collateral breakage** | For edit tasks: the fraction of runs that break at least one regression case, and the number of files changed outside the ones the task needs. | H3 |
| **Review accuracy** | For review tasks: the planted bug found (right file and line range, right explanation), false positives reported, and tokens used. | H5 |
| **Foreign-syntax intrusions** | Compile errors caused by writing another language's syntax or std names (Rust's `::`, TypeScript's `=>` on a function, Go's `:=`). Classified by a script on the diagnostic code plus a model check of the source span. Measures the "familiar surface" principle directly: familiar enough to transfer, distinct enough not to mislead. | Design feedback |
| **Meaning guesses** | Places where the agent writes, in its transcript, that the docs didn't settle something and it guessed. Continues the spec agent-test metric. | Design feedback |

### Triage of Quince failures

During implementation the most useful output is not a number. Every failed Quince run in Phases 1 to 3 is triaged, first by a model and then confirmed by a person, into one cause:

- **Spec gap:** the docs don't settle the question (goes to David as a spec question).
- **Diagnostic:** the error was right but didn't lead to the fix (goes to `quince_diagnostics`).
- **Std gap:** a function an agent reasonably expected is missing (goes to `std/API.md`).
- **Toolchain bug:** the compiler or runtime was wrong (gets a conformance test and a fix).
- **Surface trap:** the agent's guess from another language compiled and meant something else, or didn't compile and the error didn't say why.
- **Model limit:** the agent would fail this in any language (the same task fails in Go too).
- **Harness or task bug:** the task is fixed, and its earlier runs are dropped from every result.

The share of each cause over time is a dashboard of its own: spec gaps and surface traps should trend to zero by 1.0.

## Statistics

- **Runs per cell.** At least 5 runs for each task, language, condition and model, and 10 for the headline comparison in Phase 4. Agents are noisy and the same task can pass and fail on the same settings.
- **Tasks dominate the variance**, not runs. The headline numbers are averages over tasks of each task's pass rate, with 95% confidence intervals from a bootstrap that resamples tasks, then runs within each task.
- **Paired comparisons.** Every task is attempted in every language, so Quince is compared to each language task by task: the mean of the per-task differences, its bootstrap interval, and a paired permutation test. A mixed-effects logistic model (task as a random effect, language and condition as fixed effects) is a cross-check in the Phase 4 report.
- **How big a difference the suite can see.** If per-task pass-rate differences between two languages have a standard deviation of about 0.3 (to be re-estimated from Phase 2), the 95% interval on the mean difference is about ±13 points with 20 tasks, ±7 with 80, and ±5 with 150. So Phase 2 can see only large effects, and the headline needs the full suite.
- **Multiple comparisons.** The five hypotheses against four comparison languages are 20 tests. The report applies a Holm correction to them and says so; every other cut of the data is labeled exploratory.
- **No peeking.** Task shares, budgets, hypotheses and the analysis script for a full run are committed before the run starts.

## Fairness

- **Toolchains** are pinned to their latest stable release at the start of each full run, and recorded.
- **Libraries.** Each language gets its standard library plus a short allowed list, preinstalled, for what its std lacks and its users would reach for: for example, Python gets `requests` and an HTTP server framework, TypeScript gets the Node built-ins and a router, Rust gets `serde`, `tokio`, `axum` and `reqwest`, Gleam gets its HTTP packages, Go needs nothing extra. Quince gets `std` only. The list is fixed per phase and published with the results.
- **Reference solutions** in every language are written to the same quality bar, and a task whose reference took much longer to write in one language is reviewed for a hidden bias in its prompt.
- **Prompts** name no construct, library or idiom, and never say "concurrently" when a limit is meant, or "safely" when a specific error is meant. The prompt states behavior.

## Contamination and holdout

The repo is the source of truth for tasks, and it may become public, so tasks committed to it will eventually be in training data. Two measures:

- **A held-out set** of about 30 tasks is kept outside the repo, in private storage, and never published. Every full run includes it. If a model does much better on the public tasks than on the held-out ones, the public tasks have leaked, and the report leads with the held-out numbers.
- **Hidden cases stay hidden even for public tasks.** They live in `bench/tasks/<id>/hidden/` in the repo for now, and move to the private store before the repo is public.

Quince's own spec and code entering training data is not contamination; it's what success looks like. The report tracks Quince's C0 score over model generations to show it happening.

## Rollout

Each phase is tied to the milestone that makes it possible ([milestones](milestones.md)).

| Phase | When | Tasks | Languages and conditions | Runs | What it's for |
| --- | --- | --- | --- | --- | --- |
| **0. Harness and seed tasks** | Now, alongside M2 to M4 | 20 written, with Go references | None run yet | 0 | The harness, case format and fake upstreams are built and tested against the Go references. None of this needs a Quince compiler. |
| **1. Check-only** | M5 exit (checker and agent tooling, no codegen) | The 20, graded as "does it type-check" and by review tasks | Quince at C1 | ~200 | First measure of the diagnostics loop (H4) and of meaning guesses on a real checker. The milestones already ask for an agent test here; this is that test, made repeatable. |
| **2. First comparison** | M9 exit (real programs with real effects) | 20 | Quince (C0, C1) and Go (C0, C1), one model | ~800 | First pass@1 and bug escape numbers. Becomes a scheduled CI job. |
| **3. Breadth** | M10 to M12 | ~80, every category | All six languages, C0 to C2, two models, the tooling ablations, the held-out set begun | ~10,000 | Every hypothesis gets a first real estimate. Edit, bug-fix, review and C interop tasks join. |
| **4. Headline** | M14, before 1.0 | ~150 plus ~30 held out | All six languages, C0 and C1 (C2 for Quince and Go), two or three models, 10 runs | ~15,000 to 20,000 | The published result, with the full statistical treatment. |

The 20 seed tasks for Phases 0 to 2 are, by category:

- **CLI (5):** `wc` with per-file errors; CSV to JSON Lines; an access-log summarizer; duplicate files by content hash; a JSON deep-merge of layered config files with conflict rules.
- **Services (4):** a key-value store with TTLs; a URL shortener that persists across restarts; a todo API with per-field validation errors; a token-bucket rate limiter in front of a fake upstream.
- **Concurrency (4):** a fan-out fetcher with a limit and per-request timeout; first-success across three mirrors with the losers cancelled; a bounded ordered pipeline; retries with backoff under a total deadline.
- **Error paths (2):** a strict config parser reporting every error with line and column; an atomic ledger.
- **Edits (3):** add pagination to the todo service starter; fix a task leak in a cancelled fan-out; extract the URL shortener's storage behind an interface.
- **Review (2):** find a missing timeout in a fan-out; find an error swallowed into a default.

Phase 1 grades only what can be graded without running code. Review tasks work in full, because they're graded on the verdict. The other tasks are graded on a clean `quince check`, which says nothing about behavior, so Phase 1 reports the diagnostics loop and meaning guesses, never a pass rate.

### In CI

Running agents on every pull request would be slow and expensive, and its noise would make the check flaky. So:

- **Every pull request** (in `cargo xtask ci`): a format check of every task's files, cheap and deterministic, and from Phase 2 the Quince reference solutions run against their hidden cases. A toolchain change that breaks a reference is a real regression, and this doubles as an end-to-end test suite for the toolchain.
- **Weekly and on each milestone's last PR:** the Phase 2 matrix, as a scheduled GitHub Actions workflow. It needs an API key stored as a repository secret, which David would add. Results are appended to a results store outside the repo (transcripts are too large for git), and a summary table is committed to `bench/results/summary.md`.
- **Full runs** (Phases 3 and 4) are launched by hand, because of their cost.

## Cost

A run of a frontier model on a 50 to 400 line task uses roughly 200,000 to 1,000,000 tokens, most of it input that prompt caching makes cheap. Phase 2's ~800 runs are a modest monthly cost on a weekly schedule. Phase 4's ~20,000 runs are the expensive one, and can be cut to about a third by running the small model on every cell but the frontier model only on C1 and on Go, Python and Quince. Exact numbers come from Phase 2's token logs, before Phase 3 is sized.

## Risks

- **The tasks favor Quince without anyone meaning to.** Mitigated by fixing category shares before Phase 3, references in every language, and review of prompts by someone fluent in each comparison language.
- **The harness grades the scaffold, not the language.** Mitigated by the minimal scaffold, the same budget everywhere, and the realistic Claude Code configuration as a second opinion.
- **Results get stale as models change.** Every result records model and date, and the report compares languages within a run, never across runs.
- **Too few tasks to say anything.** The power estimate above is honest about what 20 tasks can show; Phase 2 is for the design loop and trends, not for claims.
- **Overfitting the language to the benchmark.** Spec changes driven by benchmark failures are fine (that's the design loop), but the guide in condition C2 is never tuned against benchmark tasks, and the held-out set catches the rest.

## Open questions for David

1. **The unseen baseline:** Gleam, as recommended, or Koka for the categories it can do as well?
2. **Hidden cases:** keep them in the repo until it goes public, or in private storage from the start?
3. **Budget:** is a weekly Phase 2 job (~800 runs) acceptable, and which models should the first full runs use?
4. **The CI secret:** an API key for the scheduled workflow, when Phase 2 starts.
5. **Disguised Go:** worth the transpiler in Phase 3, or only if Quince and Gleam come out close?

## Task list

- [ ] Phase 0: write the harness design doc in `docs/design/` and run the design review on it.
- [ ] Phase 0: build `bench/harness` (process cases, HTTP cases, fake upstreams) and the task format check in `cargo xtask ci`.
- [ ] Phase 0: write the 20 seed tasks with Go references; run each reference 3 times to catch flaky cases.
- [ ] Phase 0: build the agent scaffold and sandbox image, and dry-run it on Go.
- [ ] Phase 1 (at M5): Quince check-only runs, failure triage, results into issues.
- [ ] Phase 2 (at M9): Quince references for the 20, the scheduled workflow, the first comparison report.
- [ ] Phase 3: grow to ~80 tasks, add the other languages, conditions, ablations and the held-out set.
- [ ] Phase 4: the full suite and the published report.
