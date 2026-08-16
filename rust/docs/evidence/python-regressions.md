# Python regression map

This file maps known Python defects to Rust checks. The Python result is evidence. It is not the expected Rust result.

| Defect ID | Python behavior | Stage 1 observable regression or blocked test |
| --- | --- | --- |
| `python-assertions` | User input checks use `assert`. Optimized Python can remove these checks. | Malformed and truncated input returns a typed error. It does not panic. |
| `python-custom-version-overlap` | `CustomVersionsEditor.versions` starts entries at the count field. | The Rust parser reads the count first. It starts entries four bytes later. A synthetic overlap case fails. |
| `python-missing-encrypt-call` | `cmd_encrypt` tests a bound method without calling it. | Stage 1 has no `encrypt` command. The command parser rejects it with exit code 2. Stage 2 must test the file-state call. |
| `python-reflective-inspect` | Inspect traverses arbitrary attributes. | Rust inspect uses only the static evidence catalog. Unknown field IDs return exit code 2. |
| `python-run-script` | The CLI imports and runs arbitrary Python. | Stage 1 has no `run_script` command. The command parser rejects it with exit code 2. |
| `python-direct-overwrite` | The edit command overwrites the input path directly. | Stage 1 commands open input read-only and preserve content, permissions, and modification time. Mutation remains blocked. |
| `python-essence-metadata-alias` | `EssenceEditor.metadata` uses a global `+0x380` descriptor and aliases later item bytes. | `essences.metadata` is not readable. An item and essence mutation test remains blocked until controlled evidence resolves the link. |
| `python-demon-capacity-conflict` | The table constant is 30. The manager iterates 24 entries. | The inventory keeps both claims. Demon reads and writes remain blocked until corpus evidence resolves capacity. |
| `python-incomplete-quests` | Quest entries contain unknown bytes and have a provisional boundary. | Quest fields are not readable. A complete-boundary parser test remains blocked. |
| `python-incomplete-compendium` | The compendium count and complete boundary are not specified. | Compendium fields are not readable. A complete-boundary parser test remains blocked. |
| `python-stats-recalculation` | General stat edits do not define game-side recalculation behavior. | Player and demon stat mutation remains blocked until load and resave tests prove recalculation rules. |
