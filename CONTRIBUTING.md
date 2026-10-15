# Contributing

Thanks for helping. Issues and pull requests are welcome.

## Setup

You need stable Rust (the minimum is in `Cargo.toml`) and [`just`](https://github.com/casey/just).
For the demo data, [DuckDB](https://duckdb.org). For `just check-wasm`, the
`wasm32-unknown-unknown` target and, on macOS, `brew install llvm`.

```sh
just            # list recipes
just test       # what you will run most
just ci         # format, lint, feature checks, tests, docs, wasm32 and packaging
```

Tests use checked-in fixtures. Downloading New York City is only needed for the city examples and
fixture regeneration. `just package` verifies that the archive compiles. It does not publish
the crate or run its test suite. Run `just test` for tests.

| Command | Purpose |
|---|---|
| `just fmt` / `just clippy` | Format / lint |
| `just doc --open` | Read the API docs |
| `just data` | Download New York City with DuckDB |
| `just resolve <id> …` | Explain rules on a New York City segment |
| `just audit` | Measure planar versus geodesic rule-boundary error across New York City |
| `just fixtures` | Regenerate the committed fixture from the extract |

On the `2026-09-23.1` New York City extract, `just audit` finds 959 of 7,413 segments with a
rule boundary more than 5 m off under planar maths. The worst, on the Jamaica Bay Greenway
cycleway, moves by about 140 m. The expected rule outcomes are in [`tests/vectors`](tests/vectors).

To see the full explanation for the README example, after `just data`:

```sh
just resolve 93212c22 --at 0.8 --heading forward --mode car
just resolve 93212c22 --at 0.8 --heading backward --mode car
```

The separate checks are `just msrv`, `just deny` (requires `cargo-deny`), and
`just schema-check` (Python 3 and network access).

## Changing behaviour

The rule semantics are the point of this project, so changes to them need care:

- If you change how a rule is evaluated, update [`docs/semantics.md`](docs/semantics.md) and
  add or adjust a case in [`tests/vectors`](tests/vectors).
- Cite the source (the Overture guide, schema, or a maintainer's answer) for any
  interpretation.
- Record it in [`CHANGELOG.md`](CHANGELOG.md).

## When the schema check fails

1. Run `just schema-check` and read the release date, schema version and enum differences.
   A download or Cargo failure means the check could not run, rather than demonstrating a
   schema incompatibility.
2. Inspect the new tagged schema and its release notes, including changes to fields,
   constraints and scoping behaviour. The automatic check compares only eight enum lists.
3. Update the model and evaluator where needed, add representative test cases, and document
   changed interpretations in `docs/semantics.md` and `CHANGELOG.md`.
4. Update `SCHEMA_VERSION` only after reviewing compatibility. Add a row to the README's
   compatibility table and keep the semantics document in sync. Re-run `just schema-check`
   and `just ci`.

A major-change-month warning is a reminder to review upcoming changes, not evidence that the
current library is incompatible. Future releases with no schema version are skipped for enum
comparison. The scheduled job runs each Monday and can also be run manually on GitHub.

## Versioning

- Supporting a new Overture schema version is released as a new minor version (0.1 to 0.2),
  because files written to the old schema may no longer load.
- The changelog entry for that release names the schema version it supports.
- Git tags mark `overrules` versions only (`v0.1.0`), not schema versions.

## Style

- `cargo fmt`, and clippy with pedantic lints, warnings as errors (`just clippy`).
- The library returns its own `Error` (`thiserror`), never `anyhow`, and never panics on bad
  input or data.
- Commit and PR titles use lowercase [Conventional Commits](https://www.conventionalcommits.org/)
  with a scope, e.g. `fix(io): preserve structured collection errors`.
- Sign off each commit with `git commit -s` (DCO sign-off).
- Work done with an AI assistant is welcome. Identify the assistant actually used with a
  `Co-authored-by:` trailer, in addition to the human sign-off.
