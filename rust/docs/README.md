# Rust port documentation

This directory contains the product and engineering source of truth for the separate Rust project under `rust/`. The original Python project remains in the parent repository and is not modified.

## Planning documents

- [Product requirements](PRD.md): product scope, CLI behavior, and acceptance criteria.
- [Architecture and porting reference](architecture.md): proposed Rust design and a map of the Python implementation.
- [Correctness strategy](correctness.md): evidence rules, test layers, fixtures, and release gates.
- [Decision log](decisions.md): accepted and proposed engineering decisions.
- [Roadmap](roadmap.md): stage order and exit criteria.

## Evidence records

- [Corpus manifest](evidence/corpus-manifest.v1.json)
- [Save-layout inventory](evidence/save-layout.v1.json)
- [Python regression map](evidence/python-regressions.md)

## Format research

These files describe the current reverse-engineering work. A statement in these files is not automatically safe to implement.

- [Original GameSave research](../../docs/GameSave.md)
- [Original SysSave research](../../docs/SysSave.md)
- [Original troubleshooting notes](../../docs/Troubleshooting.md)
- [Original experimentation notes](../../docs/experimentation/)
- [Original random offsets](../../docs/random_offsets.md)

Use the evidence states in [Correctness strategy](correctness.md#2-evidence-states) before code uses an offset or invariant. Text such as `TODO`, `maybe`, `unknown`, and `???` identifies research data, not a product requirement.

## Document authority

When documents conflict, use this order:

1. Confirmed behavior from a reproducible test and a recorded save corpus.
2. `correctness.md` release gates.
3. Accepted entries in `decisions.md`.
4. `PRD.md` product behavior.
5. `architecture.md` design.
6. Existing format and experimentation notes.
7. Current Python behavior.

The Python application is evidence. It is not the specification. Agreement with Python alone does not prove correctness.
