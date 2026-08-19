# Decision log

This file records product and architecture decisions for the Rust port.

Status values:

- **Accepted:** implementation must follow the decision.
- **Proposed:** review is still open. Do not build a hard-to-reverse dependency on it.
- **Superseded:** a later decision replaces it.
- **Rejected:** do not use the option unless new evidence reopens the decision.

## D-001: Rewrite by verified behavior

- Date: 2026-08-15
- Status: Accepted
- Decision: Reimplement confirmed behavior in Rust. Do not translate the Python class and descriptor structure line by line.
- Reason: The Python repository has no automated tests and contains explicit unknowns, partial features, unsafe assertions, and known CLI defects. A structural port can preserve these defects.
- Consequence: Each ported field needs an evidence state and an independent correctness check. Some current Python features will not exist in the first Rust release.

## D-002: Treat Python as evidence, not specification

- Date: 2026-08-15
- Status: Accepted
- Decision: Python output can be a differential oracle only after independent evidence confirms the underlying format claim.
- Reason: Two implementations can agree on the same wrong offset, range, or invariant.
- Consequence: A Python/Rust mismatch starts an investigation. Rust does not change only to make the mismatch disappear.

## D-003: Preserve unknown bytes

- Date: 2026-08-15
- Status: Accepted
- Decision: Parse only required fields and edit a copy of the original decrypted byte buffer in place. Preserve all unowned bytes exactly.
- Reason: Large parts of the save remain unknown. Re-serializing a partial model can destroy data.
- Consequence: Every mutation declares exact owned ranges. A decrypted diff outside those ranges is a test failure.

## D-004: Use exact format profiles

- Date: 2026-08-15
- Status: Accepted
- Decision: Match exact profiles by length, markers, integrity, and structural checks. Reject an unknown layout.
- Reason: Offset-based editing on a guessed version can corrupt a valid save.
- Consequence: A new game build or platform layout needs corpus evidence and a new or expanded profile. There is no best-effort write mode.

## D-005: Start with one Cargo package

- Date: 2026-08-15
- Status: Accepted
- Decision: Build one package with a library and one CLI binary.
- Alternatives: A multi-crate workspace; a CLI-only binary.
- Reason: A library boundary separates format logic from presentation. A workspace adds versioning and build structure without a second product.
- Consequence: Split a crate only when an independent consumer or dependency boundary proves the need.

## D-006: Use checked typed byte access

- Date: 2026-08-15
- Status: Accepted
- Decision: Route all save-buffer reads and writes through checked primitive accessors with explicit little-endian decoding.
- Alternatives: Packed Rust structs; direct indexing; a generic reflection framework.
- Reason: Packed structs introduce alignment and layout risks. Direct indexing repeats bounds logic. Reflection hides field ownership.
- Consequence: Domain modules refer to named profile ranges and typed accessors. User input cannot reach unchecked indexing.

## D-007: Keep the stable CLI static

- Date: 2026-08-15
- Status: Accepted
- Decision: Use a fixed command tree and field registry. Do not port reflective attribute traversal.
- Reason: Static commands provide stable help, validation, completion, JSON contracts, and error behavior.
- Consequence: A new public field requires an explicit registry entry, evidence state, parser, renderer, and tests.

## D-008: Do not port arbitrary scripts or plug-ins initially

- Date: 2026-08-15
- Status: Accepted
- Decision: Exclude Python script execution and dynamic plug-ins from the first stable Rust release.
- Reason: Arbitrary code execution bypasses field validation and makes the safety contract false. A plug-in ABI would freeze an architecture before the format is stable.
- Consequence: Use JSON output, stable exit codes, and shell composition for automation. Consider a constrained batch format before a plug-in ABI.

## D-009: Make writes explicit and transactional

- Date: 2026-08-15
- Status: Accepted
- Decision: Require an output path or `--in-place`. An in-place write creates a backup and uses a temporary file plus rename.
- Alternatives: Rewrite the input by default; update bytes with random-access writes.
- Reason: Save corruption has a high cost. A partial process or storage failure must not destroy the only source file.
- Consequence: Output collisions, path aliasing, backup failure, and inability to provide safe replacement are hard errors.

## D-010: Separate detection, validation, and repair

- Date: 2026-08-15
- Status: Accepted
- Decision: Detection identifies a profile. Validation reports integrity and structure. Mutation updates a hash as part of an approved output. Load does not silently repair.
- Reason: Silent hash updates can make damaged or unsupported data appear valid.
- Consequence: `validate` is read-only. A bad hash blocks normal edits. Any future repair command needs its own requirements and warnings.

## D-011: Defer incomplete mutation domains

- Date: 2026-08-15
- Status: Accepted
- Decision: Do not initially write demon creation, compendium, quests, position, stats, linked item/essence state, or other incomplete structures.
- Reason: Current notes identify unknown bytes, linked state, or game-side reset behavior in these domains.
- Consequence: Research and confirmed read-only output can continue. Stable write support starts only after the `confirmed-write` gate passes.

## D-012: Use a change journal, not full-buffer snapshots

- Date: 2026-08-15
- Status: Accepted
- Decision: Record the original and replacement bytes for each mutation range. Use the journal for rollback and reporting.
- Alternatives: Clone the full decrypted save before every operation; apply changes only during final serialization.
- Reason: The save fits in memory, but a full clone for each edit is unnecessary. A small journal makes byte ownership visible.
- Consequence: The mutation engine records old and new range bytes, rejects overlaps, applies one complete plan to a private working buffer, and publishes output only after validation.

## D-013: Use a mature Rust CLI parser and pure report renderers

- Date: 2026-08-15
- Status: Accepted
- Decision: Use `clap` derive for argument parsing. Render text and JSON from shared report types.
- Alternatives: Manual parsing; separate command implementations for text and JSON.
- Reason: A mature parser provides consistent help and value errors. Shared reports prevent behavior differences between output formats.
- Consequence: The implementation uses `clap` derive and shared report types. Rust 1.85 is the minimum supported version.

## D-014: Use established cryptographic crates

- Date: 2026-08-15
- Status: Accepted
- Decision: Use maintained RustCrypto crates for AES, ECB block handling, and SHA-1. Do not implement cryptographic primitives.
- Reason: The save format requires established algorithms. Custom primitives add correctness and security risk without product value.
- Consequence: Pin normal semver ranges, review dependency provenance, and verify with standard known-answer vectors and real corpus pairs.

## D-015: Keep real save files out of Git by default

- Date: 2026-08-15
- Status: Accepted
- Decision: Store only privacy-reviewed and redistribution-approved saves in the repository. Use a private corpus for other integration cases.
- Reason: Save files can contain personal data and copyrighted game data.
- Consequence: Public manifests use anonymous IDs and digests. CI must not print personal field content.

## D-016: No unsafe Rust in the initial implementation

- Date: 2026-08-15
- Status: Accepted
- Decision: Do not use `unsafe` Rust in the initial parser, editor, CLI, or I/O code.
- Reason: The file is small, and checked safe Rust can meet the performance requirement.
- Consequence: Any future unsafe block needs a separate accepted decision, a stated invariant, tests, and a measured need.
