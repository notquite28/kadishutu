mod support;

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

fn binary() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("kadishutu"))
}

#[test]
fn opaque_block_aligned_input_is_unrecognized_and_skips_sha1() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("opaque.sav");
    fs::write(&path, vec![0_u8; support::synthetic::SAVE_LENGTH]).unwrap();
    let output = binary()
        .args(["validate", path.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(4));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["data"]["input_kind"], "unrecognized");
    assert_eq!(report["data"]["sha1"], "not_run");
    assert_eq!(report["data"]["profile"], Value::Null);
    assert!(!output.stderr.is_empty());
}

#[test]
fn structurally_identified_bad_hash_exits_five_without_repair() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("bad-hash.sav");
    let mut bytes = support::synthetic::valid_pc_profile();
    bytes[0x500] ^= 1;
    fs::write(&path, &bytes).unwrap();
    binary()
        .args(["validate", path.to_str().unwrap(), "--format", "json"])
        .assert()
        .code(5)
        .stdout(predicate::str::contains("\"sha1\":\"fail\""));
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn evidence_approved_pc_profile_reports_success() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("candidate.sav");
    fs::write(&path, support::synthetic::valid_pc_profile()).unwrap();
    binary()
        .args(["validate", path.to_str().unwrap(), "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"profile\":\"smtvv-pc-gamesave-449680\"",
        ))
        .stdout(predicate::str::contains("\"platform_evidence\":[\"pc\"]"))
        .stdout(predicate::str::contains("\"structure\":\"pass\""));
}

#[test]
fn candidate_get_is_one_json_error_object_and_exit_two() {
    let output = binary()
        .args(["get", "unused.sav", "game.macca", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["command"], "get");
    assert_eq!(report["ok"], false);
    assert_eq!(report["error"]["kind"], "cli_value");
    assert!(!output.stderr.is_empty());
}

#[test]
fn missing_input_is_json_io_error_and_exit_three() {
    let output = binary()
        .args(["validate", "does-not-exist.sav", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["error"]["kind"], "io");
}

#[test]
fn stdin_is_bounded_and_non_gvas_is_unrecognized() {
    binary()
        .args(["validate", "-", "--format", "json"])
        .write_stdin(vec![0_u8; 16])
        .assert()
        .code(4)
        .stdout(predicate::str::contains("\"input_kind\":\"unrecognized\""));
}

#[test]
fn stdin_rejects_maximum_length_plus_one() {
    binary()
        .args(["validate", "-", "--format", "json"])
        .write_stdin(vec![0_u8; support::synthetic::SAVE_LENGTH + 1])
        .assert()
        .code(4)
        .stdout(predicate::str::contains(
            "input exceeds maximum supported length",
        ));
}

#[test]
fn unreleased_mutation_fields_are_rejected_by_set() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("input.sav");
    let output = directory.path().join("output.sav");
    let bytes = support::synthetic::valid_pc_profile();
    fs::write(&input, &bytes).unwrap();

    let result = binary()
        .args([
            "set",
            input.to_str().unwrap(),
            "game.macca",
            "100",
            "--output",
            output.to_str().unwrap(),
            "--dry-run",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["command"], "set");
    assert_eq!(report["error"]["kind"], "cli_value");
    assert_eq!(fs::read(input).unwrap(), bytes);
    assert!(!output.exists());
}

#[test]
fn old_mutation_commands_remain_absent() {
    for command in ["edit", "gui", "run_script", "update_hash"] {
        binary().arg(command).assert().code(2);
    }
}

#[test]
fn help_is_successful() {
    binary()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("validate").and(predicate::str::contains("inspect")));
}
