# Rust port roadmap

Status: Proposed  
Date: 2026-08-15

Implementation state: Stages 0, 1, and 2 pass for the PC profile `smtvv-pc-gamesave-449680`. Switch support remains deferred.

This roadmap is evidence-gated. A stage ends when its exit criteria pass. A calendar date does not override a failed correctness gate.

## Stage 0: Preserve and classify current knowledge

### Work

- Create an inventory of every Python read and write range.
- Assign an evidence state to each field and invariant.
- Record platform, game build, and source for current data tables and offsets.
- Build an anonymous corpus manifest.
- Collect at least two encrypted/decrypted real save pairs for each platform that the project plans to claim.
- Record known Python defects as regression cases.
- Resolve broken internal documentation links that affect format research.

### Exit criteria

- Every current mutation is `confirmed-write`, `candidate`, `experimental`, `unknown`, or `rejected`.
- No field is marked confirmed only because Python uses it.
- Corpus privacy and redistribution policy is recorded for every case.
- Switch and PC support claims are separate when evidence differs.

## Stage 1: Rust foundation and read-only CLI

### Work

- Create one Cargo package with a library and CLI binary.
- Add typed errors, checked byte access, and exact profile detection.
- Implement read-only envelope and UE header parsing needed for detection.
- Implement SHA-1 calculation and validation.
- Add `validate`, `inspect`, and `get`.
- Add text and versioned JSON reports.
- Add unit, parser, decrypted no-op, CLI, and fuzz test targets.

### Exit criteria

- The read-only preview gate in [correctness.md](correctness.md#81-read-only-preview) passes.
- Read-only commands do not open inputs for writing.
- Malformed corpus and fuzz inputs do not panic.
- Unsupported formats produce exit code 4.
- Integrity failures produce exit code 5.

## Stage 2: Cryptography and explicit conversion

### Work

- Implement AES-256-ECB through maintained cryptographic crates.
- Verify standard known-answer vectors.
- Verify real encrypted/decrypted save pairs.
- Add `decrypt` and `encrypt` with explicit output paths.
- Add output collision, path alias, temporary-file, and source-preservation tests.
- Prove exact no-op ciphertext identity.

### Exit criteria

- The crypto release gate in [correctness.md](correctness.md#82-crypto-release) passes.
- Decrypt and encrypt never rewrite the source by default.
- A failed operation leaves no output that appears complete.
- Supported profile and platform claims are explicit in release output.

## Stage 3: Transactional mutation framework

### Work

- Keep the static evidence field registry as the write gate.
- Implement mutation plans, exact decrypted byte ownership, and the change journal.
- Update the SHA-1 field at `0x00..0x14` from bytes `0x40..EOF`.
- Reparse and fully validate each proposed output.
- Implement `set`, `--dry-run`, explicit output, and structured change reports.
- Preserve the input encryption state in the output.

Use an internal test field until a real field reaches `confirmed-write`. Do not release the internal field as save support. Keep `--in-place` deferred until backup and atomic replacement behavior passes on Linux, Windows, and macOS.

### Exit criteria

- Dry run performs all checks and no writes.
- Failed mutation, backup, or rename leaves the source unchanged.
- Diff enforcement rejects every undeclared byte change.
- A repeated idempotent set produces no semantic or byte change.

## Stage 4: First low-risk edits

### Candidate order

1. Macca.
2. Glory.
3. Play time.

The order can change when evidence shows different risk. Do not group candidates into one release gate.

### Work for each field

- Complete its evidence record.
- Define storage and accepted game ranges.
- Test two or more controlled source states.
- Pass decrypted diff ownership checks.
- Pass in-game load, observation, and game-resave checks on each claimed platform.
- Add minimum, maximum, normal, invalid, and idempotence tests.
- Publish the stable field ID only after the gate passes.

### Exit criteria

- The first mutation release gate in [correctness.md](correctness.md#83-first-mutation-release) passes for each released field.
- No candidate field is enabled because another field passed.

## Stage 5: Linked and normalized edits

Work on one domain at a time:

- player names and save-screen copies;
- cycles and endings copies;
- items and essence metadata;
- team order and summoned-state fields;
- player level, experience, and stat state;
- demon stats and skill state;
- position, map, rotation, and layline state.

### Required work

- Identify authoritative and cached fields.
- Define one semantic transaction for all required ranges.
- Record game normalization and reset behavior.
- Test state transitions, not only static values.
- Keep write support disabled when one linked range remains unknown.

### Exit criteria

Each domain independently reaches `confirmed-write`. There is no stage-wide assumption that all linked domains are safe.

## Stage 6: Incomplete format research

Research targets:

- complete demon records and safe creation;
- compendium entry layout;
- quest entry layout and transitions;
- map and story progress data;
- SysSave profile and semantics;
- build-specific and platform-specific layout changes.

### Method

- Produce controlled save pairs.
- Keep raw experiments separate from confirmed reference data.
- Promote a claim only through the evidence states.
- Add a profile when a build changes ranges or invariants.

### Exit criteria

A target moves to a product stage only when its unknown bytes cannot affect the proposed operation and its read or write gate passes.

## Stage 7: Stable release

### Work

- Pass the stable release gate.
- Publish supported game builds, platforms, profiles, and writable fields.
- Publish CLI and JSON compatibility policy.
- Produce signed or checksummed release archives.
- Run recovery and in-game acceptance procedures from clean corpus copies.

### Exit criteria

- All functional acceptance criteria in [PRD.md](PRD.md#9-functional-acceptance-criteria) pass.
- There is no open save-corruption defect.
- Every public write field has current evidence.
- The stable binary has no runtime Python dependency.

## Deferred product work

These items need a new requirement and decision before implementation:

- GUI;
- plug-in ABI;
- arbitrary scripts;
- a repair command;
- original SMT V support;
- remote or cloud save access;
- automatic game-data extraction.

## First implementation backlog

Start implementation with these concrete tasks after Stage 0 evidence is available:

1. Create the Cargo package and CI build matrix.
2. Add checked byte range and little-endian primitive access.
3. Add synthetic SHA-1 known-answer tests.
4. Add exact decrypted GameSave profile detection.
5. Add private corpus manifest loading for local integration tests.
6. Implement read-only validation reports.
7. Add the `validate`, `inspect`, and `get` commands with exit-code tests.
8. Add read-only and decrypted no-op tests.
9. Add fuzz targets for detection and header parsing.
10. Implement explicit decrypt and encrypt outputs after all crypto gates pass.
