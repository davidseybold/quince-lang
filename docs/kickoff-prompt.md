# First prompt for Claude Code

Paste this into `claude` from the repo root to start M0.

Historical: this started M0 on 2026-11-10. OpenSpec has since been dropped; design docs live in `docs/design/`.

---

Read CLAUDE.md, then docs/repo-plan.md and docs/milestones.md, then the spec (spec/quince-spec.md) in full.

We're starting milestone M0 (repo and harness). Please:

1. Set up OpenSpec in this repo (`openspec init`), and write M0 as the first change, with a proposal, design and tasks, before writing code. Show me the proposal before you implement.
2. Create the Cargo workspace with the crate layout from the repo plan (empty crates are fine for now), plus `xtask` with `cargo xtask ci` running fmt, clippy and tests.
3. Propose a scheme for stable rule IDs in the spec, and show me a sample of one section before applying it to the whole spec.
4. Extract the grammar appendices into spec/grammar.ebnf, and add a check that it matches the spec text.
5. Build the conformance runner and the diagnostics crate skeleton, as M0 describes.

Stop and ask me whenever the spec is unclear. Don't guess and don't edit the spec without asking.
