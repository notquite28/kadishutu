# Repository Guidelines

## Project Overview

Kadishutu is a save editor for *Shin Megami Tensei V: Vengeance*. The active product is a Rust CLI and library. It detects, validates, decrypts, inspects, mutates, and encrypts supported saves without changing unknown bytes.

The Python application in `../src/kadishutu/` is legacy evidence in the parent repository. Do not modify it, line-by-line port it, or treat it as the product specification. Use the authority order in `docs/README.md`: reproducible tests and recorded corpus evidence, correctness gates, accepted decisions, PRD, architecture notes, then Python behavior.

## Architecture & Data Flow

Rust entry points are `src/main.rs` (binary) and `src/lib.rs` (library). The CLI layer in `src/cli.rs` parses commands and controls safe input/output handling. Core modules include:

- `src/detect.rs`: identify supported save profiles.
- `src/unreal.rs`: parse Unreal save structure.
- `src/crypto.rs`: encryption, decryption, and SHA-1-related invariants.
- `src/report.rs`: stable inspection/report output.
- `src/mutation.rs`: checked edits, owned ranges, diffs, and preservation rules.

Expected flow: read input -> detect and validate profile -> parse -> inspect or apply a checked mutation -> update integrity data -> encrypt when requested -> write through the transactional output path. Keep validation read-only. Reject unknown, unsupported, or uncertain edits. Preserve bytes outside owned ranges.

The legacy Python flow is useful only as evidence: `../src/kadishutu/main.py` dispatches CLI and GUI work; `core/shared/file_handling.py` manages decrypt, hash, encrypt, and write behavior; `core/game_save/` mutates one shared `bytearray` through fixed-offset editors.

## Key Directories

- `src/`: Rust product code. Keep the public library and CLI behavior aligned.
- `tests/`: Rust integration tests and deterministic synthetic-save support.
- `tests/support/synthetic.rs`: reusable deterministic valid-save fixture.
- `fuzz/`: independent `cargo-fuzz` package and five fuzz targets.
- `docs/`: Rust-port requirements, decisions, correctness rules, and evidence schemas.
- `docs/evidence/`: corpus/layout manifests and JSON schemas. Do not add real saves.
- `tools/evidence.py`: build and verify privacy-preserving evidence manifests.
- `../src/kadishutu/`: original Python implementation and data. Read it as evidence only. Do not modify it from Rust work.

## Development Commands

Use Cargo for the Rust product. Rust 1.85 is the MSRV; keep `Cargo.lock` valid and use `--locked` for normal checks.

```sh
cargo build --locked
cargo test --locked --all-targets
cargo run --locked -- <command> [arguments]
```

The workflow template at `.github/workflows/rust.yml` is kept inside this subtree for separation. GitHub does not activate nested workflow directories. Copy it to the parent repository's `.github/workflows/` directory only when the maintainers choose to activate Rust CI. Its commands use `rust/` as the working directory.

The parent Python package uses Poetry (`../pyproject.toml`, `../poetry.lock`) and supports Python `>=3.9,<3.14`. Parent README commands such as `pipx install ...` and `kadishutu gui ...` target the original application, not the Rust workflow.

## Code Conventions & Common Patterns

- Use Rust naming conventions: `snake_case` for functions, modules, and tests; `PascalCase` for types; `SCREAMING_SNAKE_CASE` for constants.
- Prefer checked little-endian byte access and explicit profile/layout validation. Do not introduce `unsafe`.
- Make mutations narrow and explicit. Maintain range ownership, no-op identity, and unknown-byte preservation.
- Keep CLI output stable. Tests assert exit status, JSON, stdout/stderr, and write behavior.
- Use real temporary files and the executable for CLI tests. Avoid mocks when an observable file or CLI contract is available.
- Keep errors actionable and fail closed for unsupported profiles, invalid fields, ambiguous inputs, and unsafe output paths.
- The Rust runtime is synchronous. Do not introduce async work without an architecture decision.
- The legacy Python code uses shared mutable buffers, offset constants, assertions for layout assumptions, and `ValueError` for invalid domain input. Do not copy its plugin or GUI architecture into Rust; accepted decisions state that initial Rust support has no scripts or plugins.

## Important Files

- `Cargo.toml`: Rust package, Rust 2024 edition, MSRV 1.85, library and binary targets.
- `Cargo.lock`: tracked dependency lockfile; CI uses `--locked`.
- `src/main.rs`, `src/lib.rs`, `src/cli.rs`: executable, public library, and CLI contract.
- `docs/PRD.md`: proposed CLI behavior and safe-output rules.
- `docs/decisions.md`: accepted architecture and safety decisions. Follow accepted entries; proposed entries are not requirements.
- `docs/correctness.md`: evidence states, mutation limits, privacy, and release proof requirements.
- `.github/workflows/rust.yml`: inactive parent-repository CI template with `rust/` as its working directory.
- `tests/corpus.rs`: ignored private-corpus release gate. It requires `KADISHUTU_CORPUS_ROOT`; never commit personal or real save data.

## Runtime/Tooling Preferences

- Use the Rust toolchain and Cargo for current product work. Keep compatibility with Rust 1.85.
- Use the Cargo lockfile. Do not replace Cargo workflow with Node, Bun, or Poetry tooling.
- Use Python only for `tools/evidence.py`, CI schema checks, or explicitly scoped legacy-Python work.
- Fuzz targets require nightly Rust and `cargo-fuzz`; they are built in CI, not run as part of the normal test command.
- `tools/evidence.py` has read-only verification commands:

```sh
python tools/evidence.py verify-layout \
  --layout docs/evidence/save-layout.v1.json \
  --schema docs/evidence/save-layout.schema.json \
  --source-root ..
```

## Testing & QA

Run the main test suite before delivering Rust behavior changes:

```sh
cargo test --locked --all-targets
```

Tests use Cargo's built-in harness plus `assert_cmd`, `predicates`, and `proptest`. Integration tests are `tests/cli.rs`, `tests/round_trip.rs`, and `tests/evidence.rs`; unit tests are near their modules. Test observable contracts: real CLI execution, output and error formats, byte identity, integrity data, permissions, output collisions, cleanup, and no unintended writes.

Use `tempfile::tempdir()` and `tests/support/synthetic.rs` for deterministic fixtures. Add a property test when arbitrary input must not panic. Add a fuzz target only for parser, crypto, report, or mutation behavior that needs arbitrary-input coverage.

No coverage tool or coverage threshold is configured. Private-corpus testing is optional and ignored by default; run it only with an authorized `KADISHUTU_CORPUS_ROOT` and keep all real saves outside Git.
