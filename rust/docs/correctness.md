# Correctness and validation strategy

Status: Proposed  
Date: 2026-08-15

## 1. Purpose

This strategy prevents a line-by-line port of incorrect or uncertain Python behavior. A feature is complete only when evidence supports the save-format claim and tests support the implementation.

Python and Rust agreement is useful differential evidence. It is not independent proof because both implementations can contain the same wrong assumption.

## 2. Evidence states

Every field, range, enum, and invariant must have one state.

| State | Meaning | Allowed product use |
| --- | --- | --- |
| `confirmed-read` | Independent evidence identifies the range, type, and interpretation. | Stable read-only output. |
| `confirmed-write` | A controlled edit passes range, game-load, and game-resave checks. | Stable mutation. |
| `candidate` | Multiple observations support the claim, but a release gate is incomplete. | Hidden research command or test only. |
| `experimental` | One observation or an uncontrolled diff suggests the claim. | Documentation and research only. |
| `unknown` | Purpose, length, coupling, or value meaning is not known. | Preserve bytes only. |
| `rejected` | Evidence disproves the claim or shows unsafe behavior. | Do not implement. Keep the reason. |

`confirmed-write` includes `confirmed-read`. Code must not infer a higher state from a lower state.

## 3. Evidence record

Store one machine-readable evidence record for each implemented field when implementation starts. The expected fields are:

```text
id
format profile
semantic name
decrypted range or ranges
primitive type and byte order
read evidence state
write evidence state
accepted values or range
linked fields and invariants
source save corpus IDs
platform and game build
experiments and results
approved tests
known unknowns
last review date
```

Do not store player names, account data, or raw save bytes in the record.

## 4. Evidence requirements

### 4.1 `confirmed-read`

A field needs all of these items:

1. At least two independently produced real saves with controlled differences, unless a public format specification defines the field.
2. A stable decrypted range across every claimed platform and game build.
3. A known primitive representation and byte order.
4. Boundary checks that do not depend on valid surrounding data by accident.
5. A semantic value that matches the game UI or another independent observation.
6. No unexplained correlated byte range that can change the interpretation.
7. A corpus test for each supported profile.

### 4.2 `confirmed-write`

A field needs all `confirmed-read` evidence and all of these items:

1. A controlled edit from at least two starting states.
2. A decrypted diff limited to approved ranges and the SHA-1 field.
3. Successful game load on each claimed platform.
4. Correct value in the game UI or game behavior.
5. A game save after load, followed by a second diff.
6. No unexplained revert, normalization, or related-state change.
7. Tests for minimum, maximum, normal, and rejected values.
8. Recovery proof: failed validation and failed output writes leave the original unchanged.

If a game save normalizes a value, document the normalization and decide whether that value is safe. Do not silently claim exact write behavior.

## 5. Corpus policy

### 5.1 Corpus classes

Use three corpus classes:

- **Public synthetic corpus:** constructed byte data for crypto, hash, bounds, and parser unit tests. It does not prove game semantics.
- **Sanitized distributable corpus:** real saves only when license, privacy, and game-content review permits distribution.
- **Private integration corpus:** real saves stored outside Git with encrypted CI access or run locally by maintainers.

A digest manifest can be public even when the corresponding save is private. It must use a collision-resistant digest such as SHA-256 and a non-personal corpus ID.

### 5.2 Required metadata

Each real corpus case records:

- corpus ID;
- platform;
- game title ID when applicable;
- game version and DLC state;
- route and approximate progression state;
- encryption state of the stored case;
- original file SHA-256;
- decrypted file SHA-256;
- expected format profile;
- expected validation result;
- known controlled values;
- provenance and redistribution permission.

### 5.3 Privacy

- Never commit a save before a privacy and redistribution review.
- Do not include player names or platform account identifiers in test logs.
- Do not upload a user save to a third-party service.
- Prefer local corpus execution when CI secret handling is not adequate.

## 6. Test layers

### 6.1 Primitive unit tests

Stage 1 tests:

- checked offset and range arithmetic;
- little-endian integer and float reads;
- UTF-8 and UTF-16LE FString lengths, terminators, and encoding;
- SHA-1 known-answer vectors and the exact covered range;
- enum handling without a default fallback.

Later mutation and cryptography stages add primitive writes, bit preservation, AES known-answer and block-length tests, and change-journal overlap and rollback tests. An encrypt-then-decrypt test is necessary but insufficient because two inverse defects can pass the same round trip.

### 6.2 Parser tests

For each profile, test:

- exact accepted length;
- one byte short and one byte long;
- truncated header;
- correct and incorrect `GVAS` marker;
- correct and incorrect SHA-1;
- unknown enum values;
- invalid UTF-16LE;
- offsets at the first and last valid byte;
- arithmetic overflow inputs;
- opaque block-aligned data that must remain unrecognized;
- ciphertext that becomes supported only after complete decrypted-profile validation.

### 6.3 No-op tests

These Stage 1 tests are release blockers:

- decrypted input remains byte-identical after every read-only operation;
- inspect and validate do not open the file for writing;
- JSON and text rendering do not mutate the document.

Stage 2 adds encrypted load, decrypt, encrypt, and exact ciphertext round-trip tests. AES-ECB is deterministic, so a no-op encrypted round trip must be exact for a supported input.

### 6.4 Mutation ownership tests

For each mutation:

1. Keep the decrypted input.
2. Apply one semantic edit.
3. Compare all decrypted bytes.
4. Allow only the operation's declared ranges and bytes `0x00..0x14` for SHA-1.
5. Verify the new hash covers bytes `0x40..end`.
6. Reopen and parse the output.
7. Verify the requested semantic value and every linked invariant.
8. Verify a second identical edit is a no-op.

The test fails when an undeclared byte changes, even if the game still loads.

### 6.5 Differential tests against Python

Run the current Python implementation only for behavior that has an independent evidence state. Compare:

- encryption and decryption output;
- SHA-1 calculation;
- values read from confirmed ranges;
- approved mutation diffs.

Classify every mismatch:

- Rust defect;
- Python defect;
- ambiguous format claim;
- corpus metadata defect.

Do not change Rust only to match Python. Resolve the mismatch with a known vector, controlled save pair, game behavior, or authoritative external format source.

Known Python defects must become regression cases. Examples include the missing method call in the encrypted-state check, unsafe assertions, and direct overwrite behavior.

### 6.6 Property and fuzz tests

Fuzz these entry points:

- encryption-state detection;
- profile detection;
- UE header and string parsing;
- every stable field reader;
- CLI value parsing;
- JSON report generation.

Properties:

- no panic, abort, hang, or unbounded allocation;
- no read or write outside the buffer;
- rejected input produces a typed error;
- successful primitive write followed by read returns the represented value;
- no-op operations preserve all bytes;
- mutation ranges never overlap unless the operation declares the overlap.

Seed fuzzing with synthetic cases and privacy-approved corpus cases.

### 6.7 CLI integration tests

Test:

- help and version output;
- every exit code;
- missing input and permission errors;
- output collision;
- temporary-file cleanup after a controlled failure;
- standard input and output restrictions;
- diagnostics only on standard error;
- one valid JSON object in JSON mode;
- dry run with no file-system changes;
- source path aliases and symbolic links;
- interruption before and after temporary-file flush where practical.

Keep `--force`, `--in-place`, backup collision, and atomic replacement tests in the deferred in-place gate.

### 6.8 In-game tests

Automated tests cannot prove that a reverse-engineered edit is accepted by the game. Each released write feature needs a recorded manual test on each claimed platform.

Procedure:

1. Record the game version, platform, DLC state, and source corpus ID.
2. Keep a protected copy of the source save.
3. Apply exactly one semantic edit.
4. Record the decrypted diff and validation report.
5. Load the edited save in the game.
6. Open the screen or trigger the behavior that displays the value.
7. Record whether the value is correct and whether related state is correct.
8. Save in the game to a new slot or protected output.
9. Decrypt and compare the game-resaved file.
10. Explain each related diff or return the feature to `candidate`.

A game launch alone is not enough. The tester must observe the edited value and related state.

Use the local evidence tool to compare two valid decrypted saves. The tool ignores the
SHA-1 field and reports only changed half-open ranges. It does not print save bytes.

```sh
python tools/evidence.py compare \
  --before /private/source.decrypted.sav \
  --after /private/changed.decrypted.sav \
  --format json
```

Change one game value between the two saves. Keep both saves private. A range report
can locate a candidate field, but it does not make that field safe to write.

## 7. Specific risk gates

### 7.1 Cryptography and integrity

Required proof:

- standard AES-256-ECB known-answer vectors;
- one public implementation cross-check;
- at least two real encrypted/decrypted save pairs;
- exact no-op ciphertext identity;
- SHA-1 vectors;
- proof that the included hash is bytes `0x00..0x14` and the covered data starts at `0x40`.

Stage 2 PC proof uses RustCrypto `aes` 0.9.2. The implementation passes the NIST AES-256 known-answer vector. OpenSSL produces the same decrypted bytes for the private cross-check. Corpus cases `pc-owner-01a`, `pc-owner-01b`, and `pc-internet-01a` decrypt to their private companions and re-encrypt to the exact original ciphertext. The public manifest records the current companion digests. The exact game build remains unknown.

### 7.2 Duplicated values

The current notes identify copies or related locations for names, difficulty, cycles, endings, DLC, and summoned state.

Before write support:

- determine which copy is authoritative;
- determine when the game synchronizes copies;
- test disagreement states without distributing them;
- define one semantic mutation that updates all required locations;
- reject a request that targets only one private copy.

### 7.3 Numeric values

Rust primitive capacity is not a safe game range. For each numeric field, record:

- storage range;
- observed game range;
- game UI limit;
- game-side normalization;
- arithmetic or linked-state effects.

Do not use examples such as `0xffff` as proof that every stored `u16` value is safe.

The legacy Macca and Glory controls accept the full unsigned 32-bit storage
range. Rust keeps that compatibility contract and reports the untested game-side
maximum in the evidence record. A Rust-edited save loaded and resaved successfully.
Normal game actions changed Macca `8210492 -> 8234422` and Glory `100 -> 10`.
This confirms game-side arithmetic and persistence for both released fields.

Play time has equal save-screen and runtime copies at `0x4FD` and `0x5D0`.
A Rust-edited save loaded with both values set to `36000` seconds. After one
tested minute and an in-game resave, both values were `36060`. A mutation must
own both four-byte ranges and reject a read when the copies disagree.

### 7.4 Stats, level, and experience

The current documentation states that demon stats can reset and that level edits can conflict with initial stats and stat points. These edits remain disabled until the project models recalculation rules and tests level-up, battle, healing, and game resave transitions.

### 7.5 Items and essences

Released consumable amounts use one item-table byte and enforce the game limits
recorded by the legacy data table. Life Stone and Medicine use `0..50`; Chakra
Drop uses `0..30`. A Rust-edited save loaded with amounts `25`, `15`, and `30`.
After one use of each item, the game resaved them as `24`, `14`, and `29`.


An essence has an amount and metadata flags. A write must preserve unknown flag bits and maintain proven coupling. The released Aogami and Nozuchi operations own both proven bytes and follow the legacy `give` and `take` flag transitions. Every candidate essence remains disabled until its item ID, metadata address, and in-game behavior pass the same gate.

Use `tools/evidence.py compare-essence` for a controlled purchase or removal:

```sh
python tools/evidence.py compare-essence \
  --before /private/before.decrypted.sav \
  --after /private/after.decrypted.sav \
  --item-id 570 \
  --format json
```

The complete identity table contains item IDs 221 through 615. Untested entries
remain `candidate`. Before the full range becomes writable, controlled in-game
tests must cover low, middle, and high item IDs and confirm both linked offsets.

### 7.6 Team and demon records

Slot order, summoned slots, several summoned-state fields, guest characters, and demon records are linked. New demon creation remains disabled until an empty record and every required linked field are known. Erasing or moving a demon also needs proof that no hidden references remain.

### 7.7 Position

A valid position needs map IDs, coordinates, rotation, and a valid layline relationship. Current notes warn that an invalid layline can return the player to the title screen. Position writes remain disabled until complete state transitions pass in-game tests.

### 7.8 Compendium and quests

The layouts contain unknown ranges and uncertain lengths. Preserve these regions. Read support can expose only confirmed subfields. Write support is blocked.

## 8. Release gates

### 8.1 Read-only preview

- Foundation unit and fuzz tests pass.
- At least one exact profile has a private real-save corpus.
- No-op tests pass.
- `validate`, `inspect`, and `get` satisfy the CLI contract.
- Reports label candidate and unknown values correctly.

### 8.2 Crypto release

- All cryptography and integrity proof in section 7.1 passes.
- Decrypt and encrypt use explicit output paths.
- Failure and collision tests prove source preservation.
- Release documentation states supported profiles, platforms, and builds.

### 8.3 First mutation release

- Each released mutation is `confirmed-write`.
- Mutation ownership tests pass.
- Atomic explicit-output tests pass on supported operating systems.
- In-game tests pass for each claimed game platform.
- The evidence matrix has no unexplained result.

In-place mutation has a separate release gate. It needs byte-identical backup proof and atomic replacement tests on Linux, Windows, and macOS.

### 8.4 Stable release

- All PRD functional acceptance criteria pass.
- Every public field has a stable ID and evidence record.
- JSON schemas are versioned and tested.
- Fuzzing has no open crash or memory-growth defect.
- No release command depends on Python.

## 9. Failure policy

When evidence conflicts:

1. Disable write support for the field.
2. Keep read support only if its interpretation remains confirmed.
3. Record the conflicting corpus IDs and game builds.
4. Add a regression test that represents the conflict when privacy permits.
5. Create a new format profile if evidence shows a layout difference.
6. Do not add a permissive fallback.

When a released edit corrupts or destabilizes a save, remove that write feature from the next patch release. Do not keep it behind a warning.
