from __future__ import annotations

import hashlib
import io
import json
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import evidence


def valid_save(
    changes: dict[int, int] | None = None,
    size: int = 128,
) -> bytes:
    data = bytearray(size)
    data[0x40:0x44] = b"GVAS"
    for offset, value in (changes or {}).items():
        data[offset] = value
    data[:20] = hashlib.sha1(data[0x40:]).digest()
    return bytes(data)


class CompareTests(unittest.TestCase):
    def test_compare_reports_only_contiguous_payload_ranges(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            before = root / "before.sav"
            after = root / "after.sav"
            before.write_bytes(valid_save())
            after.write_bytes(valid_save({80: 1, 81: 2, 96: 3}))

            stdout = io.StringIO()
            with redirect_stdout(stdout):
                evidence.compare(before, after, "json")

            report = json.loads(stdout.getvalue())
            self.assertEqual(report["changed_byte_count"], 3)
            self.assertEqual(
                report["changed_ranges"],
                [
                    {"start": 80, "end": 82, "length": 2},
                    {"start": 96, "end": 97, "length": 1},
                ],
            )
            self.assertEqual(report["integrity_range_ignored"], {"start": 0, "end": 20})

    def test_compare_rejects_invalid_hash(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            before = root / "before.sav"
            after = root / "after.sav"
            before.write_bytes(valid_save())
            invalid = bytearray(valid_save())
            invalid[90] = 1
            after.write_bytes(invalid)

            with self.assertRaisesRegex(evidence.EvidenceError, "invalid SHA-1"):
                evidence.compare(before, after, "text")

    def test_compare_rejects_different_lengths(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            before = root / "before.sav"
            after = root / "after.sav"
            before.write_bytes(valid_save())
            longer = bytearray(144)
            longer[0x40:0x44] = b"GVAS"
            longer[:20] = hashlib.sha1(longer[0x40:]).digest()
            after.write_bytes(longer)

            with self.assertRaisesRegex(evidence.EvidenceError, "lengths differ"):
                evidence.compare(before, after, "text")


    def test_compare_essence_reports_linked_and_unrelated_changes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            before = root / "before.sav"
            after = root / "after.sav"
            owned = 0x4C72 + 221
            metadata = owned + 0x380
            before.write_bytes(valid_save({metadata: 0x16}, size=22_000))
            after.write_bytes(
                valid_save({owned: 1, metadata: 0x06, 21_500: 1}, size=22_000)
            )

            stdout = io.StringIO()
            with redirect_stdout(stdout):
                evidence.compare_essence(before, after, 221, "json")

            report = json.loads(stdout.getvalue())
            self.assertEqual(report["owned_transition"], {"before": 0, "after": 1})
            self.assertEqual(
                report["metadata_transition"], {"before": 0x16, "after": 0x06}
            )
            self.assertEqual(report["target_offsets_changed"], [owned, metadata])
            self.assertEqual(report["unrelated_changed_byte_count"], 1)
            self.assertFalse(report["linked_only"])

class LayoutRangeTests(unittest.TestCase):
    def test_linked_ranges_must_be_ordered_and_disjoint(self) -> None:
        evidence.checked_range(
            {
                "id": "essences.example.owned",
                "address": {
                    "kind": "linked",
                    "ranges": [
                        {"start": 100, "end": 101},
                        {"start": 200, "end": 201},
                    ],
                },
            }
        )

        with self.assertRaisesRegex(evidence.EvidenceError, "unordered or invalid"):
            evidence.checked_range(
                {
                    "id": "essences.example.owned",
                    "address": {
                        "kind": "linked",
                        "ranges": [
                            {"start": 200, "end": 201},
                            {"start": 100, "end": 101},
                        ],
                    },
                }
            )

if __name__ == "__main__":
    unittest.main()
