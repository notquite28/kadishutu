# Product requirements: Rust save editor

Status: Draft  
Date: 2026-08-15

## 1. Problem

The current Python application can decrypt and edit Shin Megami Tensei V: Vengeance saves. It also contains uncertain offsets, partial feature support, unsafe assertions, and a CLI that can overwrite a file without a transaction. The project has no automated test corpus. The current README states that the CLI needs replacement and that the core needs a Rust rewrite after the format is better understood.

A direct line-by-line port can preserve incorrect behavior. The Rust port must treat the Python code as one research source. It must not treat the Python code as the format specification.

## 2. Product goal

Deliver a safe, scriptable Rust CLI that can inspect, validate, decrypt, encrypt, and edit supported SMT V: Vengeance save files. The tool must preserve every byte that a requested edit does not own. The tool must reject unsupported or uncertain operations.

## 3. Users

- Players who need a safe local save edit.
- Researchers who compare save files and test format hypotheses.
- Script authors who need stable JSON output and exit codes.
- Maintainers who add a field only after they collect sufficient evidence.

## 4. Goals

### 4.1 Safety

- Validate the file envelope before an operation reads or changes a field.
- Reject an unknown file size, format, or field state by default.
- Never modify the source file unless the user supplies `--in-place`.
- Create a backup before an in-place change.
- Write to a temporary file, flush it, and rename it into place.
- Preserve unknown bytes.
- Validate the output before a write becomes visible.
- Report the exact fields and decrypted byte ranges that changed.

### 4.2 Correctness

- Implement only features that meet the evidence rules in [correctness.md](correctness.md).
- Keep format parsing independent from CLI presentation.
- Use explicit little-endian integer and float decoding.
- Check every offset, length, enum value, index, and numeric range.
- Maintain linked values as one operation only when evidence proves the link.
- Make a no-op encrypted or decrypted round trip byte-identical.

### 4.3 CLI quality

- Provide stable help text, exit codes, and JSON output.
- Use explicit input and output paths.
- Keep read-only commands read-only.
- Show a dry-run change plan before a mutation when requested.
- Distinguish invalid input, unsupported format, invalid requested value, and I/O failure.
- Never use Python-style reflective property traversal.
- Never use language assertions for user input validation.

## 5. Non-goals for the first stable release

- A GUI.
- SysSave editing.
- Arbitrary scripts or dynamic plug-ins.
- New demon creation or demon block reconstruction.
- Quest or compendium mutation.
- Mutation of an offset that has only experimental evidence.
- Automatic repair of a damaged save.
- Support for original Shin Megami Tensei V saves.
- A promise that Switch and PC saves are identical without corpus evidence.

Read-only research output for a non-goal is allowed only when it cannot imply that the format is supported.

## 6. File support

The first released profile is `smtvv-pc-gamesave-449680`. It accepts the evidence-backed PC SMT V: Vengeance `GameSave` layout of 449,680 bytes. Detection also checks the decrypted `GVAS` marker at offset `0x40`, the exact UE header signature, the SHA-1 field, and structural bounds. File size alone is not format detection.

The implementation keeps platform support claims separate:

- Switch: unsupported until the Switch release gate passes.
- PC: supported by two independent source groups under the read-only release gate.
- SysSave: unsupported until a separate exact profile and corpus exist.

## 7. CLI contract

The executable name is `kadishutu`.

### 7.1 Commands

The CLI exposes these commands:

```text
kadishutu validate <FILE> [--format text|json]
kadishutu inspect <FILE> [--field <PATH>]... [--format text|json]
kadishutu get <FILE> <FIELD> [--format text|json]
kadishutu decrypt <INPUT> --output <OUTPUT> [--format text|json]
kadishutu encrypt <INPUT> --output <OUTPUT> [--format text|json]
kadishutu set <INPUT> <FIELD> <VALUE> --output <OUTPUT> [--dry-run] [--format text|json]
kadishutu set-many <INPUT> --set <FIELD=VALUE>... --output <OUTPUT> [--dry-run] [--format text|json]
```

`set` and `set-many` are mutation transaction boundaries. They accept only fields with `confirmed-write` evidence and registered mutation operations. `set-many` rejects duplicate fields and applies every assignment in one mutation plan, SHA-1 update, encryption, and atomic write. Released linked essence fields are listed in section 7.4.

### 7.2 Shared behavior

- `-` can mean standard input for read-only commands. Conversion and mutation commands require file paths.
- An output path must not exist and must not refer to its input.
- Conversion and mutation commands write to a temporary file in the destination directory, flush and sync the file, and atomically persist it without replacing an existing file.
- Conversion and mutation commands do not overwrite the input.
- Mutation output keeps the input encryption state. The command does not infer encryption state from a file name.
- `--dry-run` performs parsing, validation, mutation planning, application, SHA-1 update, decrypted diff enforcement, and complete output validation. It does not write a file.
- `--in-place`, `--force`, and backup behavior remain unavailable until their cross-platform release gates pass.
- Text output goes to standard output. Diagnostics go to standard error.
- JSON mode must emit one documented JSON object and no extra standard-output text.
- Secret or personal save content, such as the player name, must not appear in diagnostics unless the user requested that field.

### 7.3 Exit codes

| Code | Meaning |
| ---: | --- |
| 0 | Success; the requested operation completed. |
| 2 | CLI syntax or value error. |
| 3 | Input or output I/O error. |
| 4 | Unsupported or unrecognized save format. |
| 5 | Integrity or structural validation failed. |
| 6 | Requested edit would violate a save invariant. |
| 7 | Output path or backup policy prevented the write. |
| 8 | Internal error. This is a defect. |

### 7.4 Field names

Field names form a versioned public API. They must come from a static registry. A `confirmed-read` field can enter read-only `inspect` and `get`. Only a `confirmed-write` field can enter a mutation command. Initial catalog examples include `game.macca`, `game.glory`, `game.play_time_seconds`, and `player.level`, but non-confirmed records are metadata only and are not readable.

Renaming or removing a released field requires a documented breaking release. Internal Rust type or module names must not leak into field names.
The released currency fields are `game.macca` and `game.glory`. They use unsigned
32-bit decimal values. Rust reads and writes the exact legacy little-endian
locations and rejects values outside `0..4294967295`.


The released linked essence fields are:

- `essences.aogami_type_1.owned` through `essences.aogami_type_7.owned`;
- `essences.aogami_type_a.owned`;
- `essences.aogami_type_b.owned`;
- `essences.aogami_type_c.owned`;
- `essences.nozuchi.owned`.

Use `0` for absent and `1` for owned. Each operation updates the item byte and
the linked metadata byte. It preserves unknown metadata bits.

`inspect` and `get` report the linked state for each released essence field:

- raw amount and metadata bytes;
- Essence Fusion availability;
- main-menu presence;
- `New` and `Owned` metadata flags;
- whether the two user-visible states are consistent.

The static Rust identity table contains all 395 legacy essence item IDs. The 11
listed fields are released. The other 384 entries remain `candidate` and cannot
be read or written through the CLI until controlled evidence covers their linked
addresses and game behavior.

### 7.5 Validation output

`validate` must report at least:

- detected encryption state;
- detected format profile and platform evidence, if known;
- file length;
- `GVAS` marker result;
- SHA-1 result for decrypted data;
- structural validation result;
- warnings for values that are valid bytes but not confirmed game states.

The command must not update a hash or change a file.

### 7.6 Change output

A successful mutation must report:

- input and output paths;
- format profile;
- input and output encryption state;
- requested field and value;
- declared owned decrypted ranges;
- exact changed decrypted ranges;
- whether the SHA-1 field changed;
- pre-write and post-write validation results;
- dry-run and output-written state.

The report must not print the AES key. The key is not a security boundary, but repeated output has no user value.

## 8. Initial feature classes

| Class | First-release intent | Condition |
| --- | --- | --- |
| Detect decrypted GameSave | Stage 1 required | Golden corpus and malformed-input tests pass. |
| SHA-1 validation | Stage 1 required | Known vectors and read-only tests pass. |
| Read-only metadata inspection | Stage 1 required | Each field has an evidence record. |
| Detect and convert encrypted GameSave | Stage 2 required | AES known vectors and encrypted round trips pass. |
| Macca and Glory edit | Candidate | Range and in-game load/save checks pass. |
| Play time edit | Candidate | Duration boundary and game resave checks pass. |
| Player name edit | Deferred | All duplicate name fields and UTF-16 limits are proven. |
| Difficulty edit | Deferred | Both observed difficulty locations are understood. |
| Cycles and endings edit | Deferred | Duplicate locations and game synchronization are proven. |
| Player and demon stats | Deferred | Recalculation and reset behavior are specified. |
| Items and essences | Deferred | Amount and metadata coupling is proven. |
| Party and demons | Deferred | All slot and summoned-state invariants are proven. |
| Position | Deferred | Map, layline, coordinate, and rotation rules are proven. |
| Compendium and quests | Research only | Current layouts contain unknown fields. |

## 9. Functional acceptance criteria

The first stable release is acceptable when all of these statements are true:

1. A supported encrypted save can pass `validate`, decrypt, and encrypt back to the exact original bytes.
2. A supported decrypted save can pass `validate` without any mutation.
3. A wrong key result, truncated file, wrong length, bad marker, and bad hash each produce a nonzero documented exit code.
4. A read-only command never changes file content or metadata.
5. A failed edit leaves the source, output, and backup state unchanged.
6. An in-place edit creates a byte-identical backup before replacement.
7. Each released edit changes only its approved decrypted byte ranges and the integrity field.
8. The edited save loads in the game on each platform that the release claims to support.
9. A load and save by the game does not revert the edited value or produce an unexplained related diff.
10. JSON output passes its committed schema and remains free of diagnostic text.
11. The CLI never panics for malformed input in the fuzz and corpus test suites.
12. The release matrix and evidence records identify every supported mutation.

## 10. Quality requirements

- Supported Rust version policy: the latest stable compiler and one documented minimum supported Rust version.
- Linux, Windows, and macOS builds must pass unit and integration tests.
- Release archives must include checksums.
- Unsafe Rust is not allowed without a separate accepted decision.
- Parser and mutation code must have no hidden network access.
- Normal operations must fit the save in memory once. The implementation must not copy the full buffer for each field access.

## 11. Open product questions

These questions block the related feature, not the read-only foundation:

- Which game builds produced the current offset map?
- Do Switch and PC use byte-identical decrypted layouts for all supported builds?
- Which fields are cached copies, and which fields are authoritative?
- What value ranges does the game accept for currency, play time, level, and stats?
- Which edits need linked changes outside the currently documented range?
- Can legal, privacy-safe real-save fixtures be distributed, or must CI use an encrypted private corpus?
