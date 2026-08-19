# Rust port documentation

This directory contains the product and engineering source of truth for the separate Rust project under `rust/`. The original Python project remains in the parent repository and is not modified.

## Current implementation

Status: Active  
Updated: 2026-08-18

The Rust application supports one exact profile:
`smtvv-pc-gamesave-449680`. This profile is the 449,680-byte PC
*Shin Megami Tensei V: Vengeance* `GameSave`. Switch saves, original SMT V
saves, and `SysSave` are not supported.

The CLI provides:

- `validate`, `inspect`, and `get`;
- explicit `decrypt` and `encrypt`;
- transactional `set` and atomic `set-many`;
- text and versioned JSON reports;
- dry runs, explicit outputs, collision rejection, and source preservation.

The released write scope has 17 stable fields:

- Macca and Glory;
- two linked play-time copies through `game.play_time_seconds`;
- Life Stone, Chakra Drop, and Medicine quantities;
- Aogami Type-1 through Type-7 and Type-A through Type-C essence state;
- Nozuchi essence state.

Released essence reads report the item amount, metadata flags, both user-visible
states, and consistency. The static essence identity table contains all 395
legacy essence IDs. The other 384 entries remain candidate-only.

Aogami Type-8 has confirmed first-acquisition evidence but remains candidate
because first acquisition includes progression state outside the released
consumption and reacquisition operation.

The application does not provide in-place writes, backups, a GUI, scripts,
plug-ins, player or demon stat writes, party edits, position edits, quest or
compendium writes, or `SysSave` support.

Current verification:

- 62 Rust tests pass and one private-corpus test is ignored by default;
- 100 evidence field records pass layout verification;
- released Macca, Glory, play-time, consumable, and linked essence edits have
  successful PC in-game load and resave evidence.

## Planning documents

- [Product requirements](PRD.md): product scope, CLI behavior, and acceptance criteria.
- [Architecture and porting reference](architecture.md): implemented Rust design and a map of the Python implementation.
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
