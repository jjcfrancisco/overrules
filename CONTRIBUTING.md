# Contributing

Thanks for helping. Issues and pull requests are welcome.

## Setup

You need Rust (stable; the minimum is in `Cargo.toml`) and [`just`](https://github.com/casey/just).
For the demo data, [DuckDB](https://duckdb.org). For `just check-wasm`, the
`wasm32-unknown-unknown` target and, on macOS, `brew install llvm`.

```sh
just            # list recipes
just test       # what you will run most
just ci         # everything CI runs, before you push
```

## Changing behaviour

The rule semantics are the point of this project, so changes to them need care:

- If you change how a rule is evaluated, update [`docs/semantics.md`](docs/semantics.md) and
  add or adjust a case in [`crates/core/tests/vectors`](crates/core/tests/vectors).
- Cite the source (the Overture guide, schema, or a maintainer's answer) for any
  interpretation.
- Record it in [`CHANGELOG.md`](CHANGELOG.md).

## Style

- `cargo fmt`, and clippy with pedantic lints, warnings as errors (`just clippy`).
- Library crates return their own `Error` (`thiserror`), never `anyhow`, and never panic on bad
  input or data.
- Commits use [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`,
  `docs:`…).
- Work done with an AI assistant is welcome. Say so with a `Co-authored-by:` trailer.
