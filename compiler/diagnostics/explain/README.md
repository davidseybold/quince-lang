# explain

One Markdown file per diagnostic code, named `<CODE>.md` (for example `E0412.md`). `quince explain <CODE>` prints it. Each file says what the diagnostic means, shows code that triggers it and the fixed version, and cites the spec rule IDs it enforces.

`cargo xtask diagnostics` fails when a file here has no row in `src/registry.rs`, when a registered code has no file or an empty one, or when no conformance fixture expects the code. See `src/registry.rs` for how to add a code.
