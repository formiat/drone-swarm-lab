# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Language

Always respond to the user in Russian. Code comments must be written in English.

## Build & Run Commands

```bash
cargo build
cargo check
cargo test
cargo test -p <crate-name>
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

## Commit Workflow

After **every** code change — without waiting for a reminder:

1. `cargo clippy --all-targets -- -D warnings` — lint
2. `cargo fmt --all` — format
3. Commit — only after both above pass

**Never push automatically or without an explicit user command.**

## Git Rules

- **Never amend commits.** If a commit was wrong, report it to the user instead of fixing.
- **Never push without an explicit user command** — no automatic push under any circumstance.
- **Never force-push** (`--force`, `--force-with-lease`, etc.).
- **Never try to fix git situations unilaterally** — report and let the user decide.
- **Never add `Co-Authored-By: Claude`** — not in code, not in commit messages.
- **Never run `cargo clean`** or delete the `target/` directory — Rust determines what to recompile. If a build fails, investigate the root cause.

## Code Style

- Prefer `///` doc comments over `//` everywhere possible.
- In `format!`, prefer named arguments: `format!("{foo}/{bar}")` not `format!("{}/{}", foo, bar)`.
- Prefer variable shadowing over introducing new names: `let x = x.trim();` not `let x2 = x.trim();`.
- Code comments must be written in English.
- When a function returns or stores a **tuple**, always add a comment describing fields:
  `/// value: \`(field1, field2)\``.
- When a map key is not obvious from the type alone, always add a comment:
  `/// key: \`agent_id\``.

## Error Handling

- `anyhow` is **forbidden**. Use typed errors with `thiserror` exclusively.
- Every public function returns a typed result — no untyped catch-all at module boundaries.

## Newtype Wrappers

Any single-field tuple struct (newtype), e.g. `struct FooId(String)`:

- Inner type must be **private**.
- Must derive: `#[derive(AsMut, AsRef, Deref, DerefMut, From, Into)]`.

## Types: Time and Duration

- Prefer `DateTime<Utc>` / `DateTime<FixedOffset>` over raw `i64` timestamp fields.
- Prefer `std::time::Duration` or `chrono::Duration` over raw `u64` seconds fields.
- Never use American date format (month/day/year) or slash separators in any output.
- Formatted timestamps: RFC 3339 for machine output (`2026-03-17T15:04:05+03:00`),
  ISO 8601 with space for human output (`2026-03-17 12:41:08+03:00`).

## Types: File Paths

- Functions that accept a path: prefer `impl AsRef<Path>` or `P: AsRef<Path>` signatures.
- For storing/passing paths: prefer `Path` / `PathBuf` over `&str` / `String`.

## Dependency Versioning

When adding an external dependency, use the **left-most non-zero** version rule:

- `0.1.23` → `"0.1"`
- `1.4.54` → `"1"`
- `0.0.5` → `"0.0.5"`

## Enum Serialization

- **New enums must always be serialized in `snake_case`** — in the DB, in JSON responses, and in config files.
- **C-style INTEGER representation for enums is deprecated.** Use string-based (`TEXT`/`VARCHAR`) storage with `snake_case` values instead.

## Reuse Before Adding

Before adding any helper (parser, serializer, converter, validator, formatter):

1. Search the repository for an existing implementation first.
2. Prefer reuse or safe extension over duplication.
3. In the final summary, state what was searched, what was reused, and what new shared code was introduced and where.

## Testing

- Cover new functionality with automated tests as much as practical.
- When changing existing functionality, add or adapt tests accordingly.
- Tests must be **self-contained and portable**: no machine-specific local files, absolute paths,
  `$HOME` content, pre-existing state, or filesystem discovery that varies by machine.
- Prefer in-memory data, inline fixtures, mock/fake/stub dependencies.
- If a file fixture is truly unavoidable, first rule out inline/in-memory alternatives, explain
  why, and use only small stable fixtures stored in the repository.
- After writing or changing tests:
  1. Verify the test is portable (no machine-specific dependencies).
  2. `cargo check -p <crate>` — must compile.
  3. `cargo test -p <crate> <test_name>` — must pass.

### Test Planning

In planning and test-planning artifacts, propose the maximum practical set of automated tests for
new and affected functionality. Group all proposed automated tests into three categories:

1. Tests that need no refactoring — mark as planned for implementation together with the main
   functional changes.
2. Tests that need light refactoring.
3. Tests that need heavy refactoring.

List all three categories explicitly. In investigations and plans, call out relevant existing
tests, coverage gaps, and what tests implementation should add or update.
