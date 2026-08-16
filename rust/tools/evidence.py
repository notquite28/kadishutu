#!/usr/bin/env python3
"""Build and verify public evidence records without exposing save contents."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path, PurePosixPath
from typing import Any

SCHEMA_VERSION = 1
METADATA_KEYS = {
    "schema_version",
    "id",
    "source_group",
    "platform",
    "title_id",
    "game_version",
    "dlc_state",
    "route_progression",
    "encrypted_file",
    "decrypted_file",
    "expected_profile",
    "expected_validation",
    "provenance",
    "redistribution",
    "privacy_reviewed",
}
PROVENANCE_KEYS = {
    "origin",
    "pairing_tool",
    "independent_validation",
    "collected_date",
}
ANONYMOUS_ID = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
EMAIL = re.compile(r"[^\s@]+@[^\s@]+\.[^\s@]+")
PRIVATE_WORDS = re.compile(r"\b(player.?name|account|username|user.?id|email|friend.?code)\b", re.I)
VALID_STATES = {"valid", "invalid", "unrecognized"}
REDISTRIBUTION = {"prohibited", "private-test-only", "approved"}


class EvidenceError(ValueError):
    """A controlled evidence file does not satisfy the public contract."""


def fail(message: str) -> None:
    raise EvidenceError(message)


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        fail(f"cannot read JSON {path}: {error}")


def write_json(path: Path, value: Any) -> None:
    text = json.dumps(value, indent=2, sort_keys=True, ensure_ascii=True) + "\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        fail(f"cannot hash corpus member {path.name}: {error}")
    return digest.hexdigest()


def relative_member(root: Path, value: Any, label: str) -> Path:
    if not isinstance(value, str) or not value:
        fail(f"{label} must be a non-empty relative path")
    posix = PurePosixPath(value)
    if posix.is_absolute() or ".." in posix.parts or "." in posix.parts:
        fail(f"{label} must not be absolute or contain traversal")
    if len(posix.parts) != 1:
        fail(f"{label} must name a member in the sidecar directory")
    candidate = root / value
    if not candidate.is_file():
        fail(f"missing corpus member referenced by {label}: {value}")
    return candidate


def non_personal(value: str, label: str) -> None:
    if EMAIL.search(value) or PRIVATE_WORDS.search(value):
        fail(f"{label} contains a prohibited personal or account identifier")


def require_text(record: dict[str, Any], key: str) -> str:
    value = record.get(key)
    if not isinstance(value, str) or not value.strip():
        fail(f"{key} must be a non-empty string")
    non_personal(value, key)
    return value

def validate_members(
    encrypted: Path,
    decrypted: Path,
    expected_validation: str,
) -> None:
    try:
        encrypted_size = encrypted.stat().st_size
        decrypted_size = decrypted.stat().st_size
        with encrypted.open("rb") as stream:
            stream.seek(0x40)
            encrypted_marker = stream.read(4)
        with decrypted.open("rb") as stream:
            stored_hash = stream.read(20)
            stream.seek(0x40)
            marker = stream.read(4)
            stream.seek(0x40)
            digest = hashlib.sha1()
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        fail(f"cannot validate corpus members: {error}")
    if encrypted_size == 0 or encrypted_size % 16:
        fail("encrypted member must be a non-empty AES-block sequence")
    if encrypted_size != decrypted_size:
        fail("encrypted and decrypted members must have the same length")
    if encrypted_marker == b"GVAS":
        fail("encrypted member unexpectedly contains the plaintext GVAS marker")
    marker_valid = marker == b"GVAS"
    hash_valid = stored_hash == digest.digest()
    if expected_validation == "valid" and not (marker_valid and hash_valid):
        fail("valid decrypted member failed its GVAS marker or SHA-1 check")
    if expected_validation == "invalid" and marker_valid and hash_valid:
        fail("invalid decrypted member unexpectedly passed marker and SHA-1 checks")
    if expected_validation == "unrecognized" and marker_valid:
        fail("unrecognized decrypted member unexpectedly contains GVAS")


def validate_metadata(path: Path, root: Path) -> tuple[dict[str, Any], Path, Path]:
    raw = load_json(path)
    if not isinstance(raw, dict):
        fail(f"{path.name} must contain an object")
    keys = set(raw)
    if keys != METADATA_KEYS:
        fail(f"{path.name} metadata keys differ: missing={sorted(METADATA_KEYS - keys)}, extra={sorted(keys - METADATA_KEYS)}")
    if raw["schema_version"] != SCHEMA_VERSION:
        fail(f"{path.name} has unsupported schema_version")
    for key in ("id", "source_group"):
        value = require_text(raw, key)
        if not ANONYMOUS_ID.fullmatch(value):
            fail(f"{key} must be an anonymous lowercase identifier")
    if raw["platform"] not in {"pc", "switch"}:
        fail("platform must be pc or switch")
    title_id = raw["title_id"]
    if title_id is not None:
        if not isinstance(title_id, str) or not title_id.strip():
            fail("title_id must be a string or null")
        non_personal(title_id, "title_id")
    require_text(raw, "game_version")
    route = require_text(raw, "route_progression")
    non_personal(route, "route_progression")
    dlc = raw["dlc_state"]
    if not isinstance(dlc, list) or any(not isinstance(item, str) or not ANONYMOUS_ID.fullmatch(item) for item in dlc):
        fail("dlc_state must be an array of anonymous identifiers")
    if len(set(dlc)) != len(dlc):
        fail("dlc_state contains duplicate identifiers")
    expected_profile = raw["expected_profile"]
    if expected_profile is not None and (not isinstance(expected_profile, str) or not ANONYMOUS_ID.fullmatch(expected_profile)):
        fail("expected_profile must be an anonymous identifier or null")
    if raw["expected_validation"] not in VALID_STATES:
        fail(f"expected_validation must be one of {sorted(VALID_STATES)}")
    if raw["redistribution"] not in REDISTRIBUTION:
        fail(f"redistribution must be one of {sorted(REDISTRIBUTION)}")
    if not isinstance(raw["privacy_reviewed"], bool) or not raw["privacy_reviewed"]:
        fail("privacy_reviewed must be true before manifest publication")
    provenance = raw["provenance"]
    if not isinstance(provenance, dict) or set(provenance) != PROVENANCE_KEYS:
        fail(f"provenance must contain exactly {sorted(PROVENANCE_KEYS)}")
    for key in PROVENANCE_KEYS - {"collected_date"}:
        require_text(provenance, key)
    if not isinstance(provenance["collected_date"], str) or not DATE.fullmatch(provenance["collected_date"]):
        fail("provenance.collected_date must use YYYY-MM-DD")
    encrypted = relative_member(root, raw["encrypted_file"], "encrypted_file")
    decrypted = relative_member(root, raw["decrypted_file"], "decrypted_file")
    if encrypted == decrypted:
        fail("encrypted_file and decrypted_file must differ")
    validate_members(encrypted, decrypted, raw["expected_validation"])
    return raw, encrypted, decrypted


def public_case(raw: dict[str, Any], encrypted: Path, decrypted: Path) -> dict[str, Any]:
    return {
        "id": raw["id"],
        "source_group": raw["source_group"],
        "platform": raw["platform"],
        "title_id": raw["title_id"],
        "game_version": raw["game_version"],
        "dlc_state": sorted(raw["dlc_state"]),
        "route_progression": raw["route_progression"],
        "encryption_state": {"encrypted_member": "encrypted", "decrypted_member": "decrypted"},
        "encrypted_sha256": sha256(encrypted),
        "decrypted_sha256": sha256(decrypted),
        "expected_profile": raw["expected_profile"],
        "expected_validation": raw["expected_validation"],
        "controlled_value_fields": [],
        "provenance": raw["provenance"],
        "redistribution": raw["redistribution"],
        "privacy_reviewed": raw["privacy_reviewed"],
    }


def collect(root: Path) -> list[tuple[dict[str, Any], Path, Path]]:
    if not root.is_dir():
        fail(f"corpus root is not a directory: {root}")
    result = []
    ids: set[str] = set()
    for sidecar in sorted(root.glob("*.metadata.json"), key=lambda item: item.name):
        raw, encrypted, decrypted = validate_metadata(sidecar, root)
        if raw["id"] in ids:
            fail(f"duplicate corpus id: {raw['id']}")
        ids.add(raw["id"])
        result.append((raw, encrypted, decrypted))
    return result


def build(root: Path, output: Path) -> None:
    cases = [public_case(*case) for case in collect(root)]
    cases.sort(key=lambda case: case["id"])
    write_json(output, {"schema_version": SCHEMA_VERSION, "cases": cases})
    print(f"wrote {len(cases)} public corpus records to {output}")


def verify(root: Path, manifest_path: Path) -> None:
    manifest = load_json(manifest_path)
    if not isinstance(manifest, dict) or set(manifest) != {"schema_version", "cases"}:
        fail("manifest must contain exactly schema_version and cases")
    if manifest["schema_version"] != SCHEMA_VERSION or not isinstance(manifest["cases"], list):
        fail("unsupported manifest schema")
    private = {raw["id"]: public_case(raw, encrypted, decrypted) for raw, encrypted, decrypted in collect(root)}
    public_ids: list[str] = []
    for case in manifest["cases"]:
        if not isinstance(case, dict) or not isinstance(case.get("id"), str):
            fail("manifest case has no valid id")
        case_id = case["id"]
        public_ids.append(case_id)
        if case_id not in private:
            fail(f"manifest member is missing from private corpus: {case_id}")
        if case != private[case_id]:
            fail(f"manifest metadata or digest mismatch for {case_id}")
    if public_ids != sorted(public_ids) or len(public_ids) != len(set(public_ids)):
        fail("manifest cases must have unique, sorted ids")
    if set(public_ids) != set(private):
        fail("private corpus and public manifest case sets differ")
    print(f"verified {len(public_ids)} private corpus records")


def checked_range(record: dict[str, Any]) -> None:
    address = record.get("address")
    if not isinstance(address, dict) or address.get("kind") not in {"absolute", "strided", "none"}:
        fail(f"field {record.get('id')} has an invalid address")
    if address["kind"] == "absolute":
        start, end = address.get("start"), address.get("end")
        if not isinstance(start, int) or not isinstance(end, int) or start < 0 or end <= start:
            fail(f"field {record.get('id')} has an invalid half-open range")
    elif address["kind"] == "strided":
        required = ("base", "stride", "count", "relative_start", "relative_end")
        if any(not isinstance(address.get(key), int) for key in required):
            fail(f"field {record.get('id')} has an invalid strided range")
        if address["base"] < 0 or address["stride"] <= 0 or address["count"] <= 0:
            fail(f"field {record.get('id')} has invalid stride arithmetic")
        if not 0 <= address["relative_start"] < address["relative_end"] <= address["stride"]:
            fail(f"field {record.get('id')} has an invalid relative range")
        last = address["base"] + address["stride"] * (address["count"] - 1) + address["relative_end"]
        if last > (1 << 63) - 1:
            fail(f"field {record.get('id')} range arithmetic overflows")


def verify_layout(layout_path: Path, schema_path: Path, source_root: Path) -> None:
    layout = load_json(layout_path)
    schema = load_json(schema_path)
    if not isinstance(schema, dict) or schema.get("$schema") is None:
        fail("layout schema is not a JSON schema")
    source_root = source_root.resolve()
    required = {"schema_version", "profiles", "corpus_ids", "experiments", "tests", "inventory_scope", "fields", "defects", "data_tables"}
    if not isinstance(layout, dict) or set(layout) != required or layout["schema_version"] != SCHEMA_VERSION:
        fail("layout has invalid top-level keys or schema version")
    fields = layout["fields"]
    if not isinstance(fields, list):
        fail("layout fields must be an array")
    field_ids = [field.get("id") for field in fields if isinstance(field, dict)]
    if len(field_ids) != len(fields) or any(not isinstance(item, str) for item in field_ids):
        fail("every field must have an id")
    if field_ids != sorted(field_ids) or len(field_ids) != len(set(field_ids)):
        fail("layout field ids must be unique and sorted")
    known_corpus = set(layout["corpus_ids"])
    known_experiments = {item["id"] for item in layout["experiments"]}
    known_tests = {item["id"] for item in layout["tests"]}
    for field in fields:
        checked_range(field)
        for key, known in (("corpus_evidence", known_corpus), ("experiment_ids", known_experiments), ("approved_test_ids", known_tests)):
            refs = field.get(key)
            if not isinstance(refs, list) or not set(refs) <= known:
                fail(f"field {field['id']} has invalid {key}")
        for source in field.get("python_sources", []):
            if not isinstance(source, dict) or not isinstance(source.get("path"), str) or not isinstance(source.get("symbol"), str):
                fail(f"field {field['id']} has an invalid source label")
            if not (source_root / source["path"]).is_file():
                fail(f"field {field['id']} references missing source {source['path']}")
    expected_sources = sorted(
        path.relative_to(source_root).as_posix()
        for path in (source_root / "src/kadishutu/core/game_save").glob("*.py")
    )
    scope = layout["inventory_scope"]
    actual_sources = sorted(
        item.get("path")
        for item in scope
        if isinstance(item, dict)
        and isinstance(item.get("path"), str)
        and item["path"].startswith("src/kadishutu/core/game_save/")
    )
    if actual_sources != expected_sources:
        fail("inventory_scope does not classify every core/game_save/*.py source")
    for item in scope:
        if item.get("classification") not in {"inventoried", "no-byte-access"}:
            fail(f"invalid inventory classification for {item.get('path')}")
    print(f"verified {len(fields)} fields and {len(scope)} inventory source records")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    build_parser = commands.add_parser("build")
    build_parser.add_argument("--root", required=True, type=Path)
    build_parser.add_argument("--output", required=True, type=Path)
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--root", required=True, type=Path)
    verify_parser.add_argument("--manifest", required=True, type=Path)
    layout_parser = commands.add_parser("verify-layout")
    layout_parser.add_argument("--layout", required=True, type=Path)
    layout_parser.add_argument("--schema", required=True, type=Path)
    layout_parser.add_argument("--source-root", type=Path, default=Path(".."))
    return result


def main() -> int:
    args = parser().parse_args()
    try:
        if args.command == "build":
            build(args.root, args.output)
        elif args.command == "verify":
            verify(args.root, args.manifest)
        else:
            verify_layout(args.layout, args.schema, args.source_root)
    except EvidenceError as error:
        print(f"evidence error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
