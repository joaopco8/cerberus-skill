# Contributing to cerberus-skill

## Getting Started

```sh
git clone https://github.com/joaopco8/cerberus-skill
cd cerberus-skill
cargo build
cargo test
```

## Code Style Requirements

All PRs must pass:

```sh
cargo fmt --check          # formatting
cargo clippy -- -D warnings  # no warnings allowed
cargo test                 # unit tests must pass
```

Run all three before opening a PR:

```sh
cargo fmt && cargo clippy -- -D warnings && cargo test
```

## No `unwrap()` in Library Code

Library code (`src/`) must not use `unwrap()`, `expect()`, or `panic!()` on
values that can fail at runtime. Use `?` and the `CerberusError` type instead.

`todo!()` and `unimplemented!()` are acceptable in stub implementations but
must be replaced before a function is considered complete.

Examples (`examples/`, `tests/`) may use `.expect()` with descriptive messages.

## Error Handling

All public functions must return `Result<_, CerberusError>`. Add new variants
to `CerberusError` in `src/error.rs` rather than using `String` errors.

## Doc Comments

Every public function, struct, and enum must have a doc comment (`///`).
Every public function must document its `# Errors` section.

## Tests

- Unit tests that do not touch the network belong in `#[cfg(test)]` blocks
  inside the relevant source file.
- Integration tests that require devnet go in `tests/devnet_e2e.rs` and must
  be annotated with `#[ignore = "requires funded devnet keypair and live RPC"]`.

## Adding Examples

New examples go in `examples/<name>.rs`. Each example must:
- Include a doc comment with the run command.
- Handle errors with `anyhow::Result` in `main`.
- Work against devnet by default.
- Accept `RPC_URL` env var for custom endpoints.

## Submitting a PR

1. Fork the repo and create a branch: `git checkout -b feat/my-feature`
2. Make your changes and ensure all checks pass (see above).
3. Open a PR against `main` with a clear description of what changed and why.
4. PRs that add new public API must include doc comments and at least one test.
