mod support;

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

use kadishutu::crypto;

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
        .args(["get", "unused.sav", "player.level", "--format", "json"])
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
            "player.level",
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
fn aogami_type_c_set_supports_dry_run_and_explicit_output() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("input.sav");
    let output = directory.path().join("output.sav");
    let mut source = support::synthetic::valid_pc_profile();
    source[0x5220] = 0x16;
    support::synthetic::update_hash(&mut source);
    fs::write(&input, &source).unwrap();
    let arguments = [
        "set",
        input.to_str().unwrap(),
        "essences.aogami_type_c.owned",
        "1",
        "--output",
        output.to_str().unwrap(),
        "--format",
        "json",
    ];

    let dry_run = binary().args(arguments).arg("--dry-run").output().unwrap();
    assert!(dry_run.status.success());
    let dry_report: Value = serde_json::from_slice(&dry_run.stdout).unwrap();
    assert_eq!(dry_report["ok"], true);
    assert_eq!(dry_report["data"]["dry_run"], true);
    assert_eq!(dry_report["data"]["output_written"], false);
    assert!(!output.exists());
    assert_eq!(fs::read(&input).unwrap(), source);

    let applied = binary().args(arguments).output().unwrap();
    assert!(applied.status.success());
    let report: Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(report["data"]["owned_ranges"][0]["start"], 0x4ea0);
    assert_eq!(report["data"]["owned_ranges"][0]["end"], 0x4ea1);
    assert_eq!(report["data"]["owned_ranges"][1]["start"], 0x5220);
    assert_eq!(report["data"]["owned_ranges"][1]["end"], 0x5221);
    assert_eq!(report["data"]["output_written"], true);

    let mut expected = source.clone();
    expected[0x4ea0] = 1;
    expected[0x5220] = 0x06;
    support::synthetic::update_hash(&mut expected);
    assert_eq!(fs::read(&output).unwrap(), expected);
    assert_eq!(fs::read(&input).unwrap(), source);
}

#[test]
fn nozuchi_set_uses_confirmed_linked_ranges() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("input.sav");
    let output = directory.path().join("output.sav");
    let mut source = support::synthetic::valid_pc_profile();
    source[0x522c] = 0x16;
    support::synthetic::update_hash(&mut source);
    fs::write(&input, &source).unwrap();

    let result = binary()
        .args([
            "set",
            input.to_str().unwrap(),
            "essences.nozuchi.owned",
            "1",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(result.status.success());
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["data"]["owned_ranges"][0]["start"], 0x4eac);
    assert_eq!(report["data"]["owned_ranges"][1]["start"], 0x522c);

    let mut expected = source.clone();
    expected[0x4eac] = 1;
    expected[0x522c] = 0x06;
    support::synthetic::update_hash(&mut expected);
    assert_eq!(fs::read(&output).unwrap(), expected);
    assert_eq!(fs::read(&input).unwrap(), source);
}

#[test]
fn inspect_reports_linked_essence_consistency_from_encrypted_save() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("input.sav");
    let mut plaintext = support::synthetic::valid_pc_profile();
    plaintext[0x4eac] = 1;
    plaintext[0x522c] = 0x16;
    support::synthetic::update_hash(&mut plaintext);
    fs::write(&input, crypto::encrypt(&plaintext).unwrap()).unwrap();

    let result = binary()
        .args([
            "inspect",
            input.to_str().unwrap(),
            "--field",
            "essences.nozuchi.owned",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(result.status.success());
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    let value = &report["data"]["fields"][0]["value"];
    assert_eq!(value["amount"], 1);
    assert_eq!(value["metadata"], 0x16);
    assert_eq!(value["fusion_available"], true);
    assert_eq!(value["main_menu_present"], false);
    assert_eq!(value["consistent"], false);
}

#[test]
fn set_many_supports_repeated_assignments_dry_run_and_duplicate_rejection() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("input.sav");
    let output = directory.path().join("output.sav");
    let mut plaintext = support::synthetic::valid_pc_profile();
    plaintext[0x5212] = 0x16;
    plaintext[0x5213] = 0x16;
    support::synthetic::update_hash(&mut plaintext);
    let encrypted = crypto::encrypt(&plaintext).unwrap();
    fs::write(&input, &encrypted).unwrap();

    let result = binary()
        .args([
            "set-many",
            input.to_str().unwrap(),
            "--set",
            "essences.aogami_type_1.owned=1",
            "--set",
            "essences.aogami_type_2.owned=1",
            "--output",
            output.to_str().unwrap(),
            "--dry-run",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(result.status.success());
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["command"], "set-many");
    assert_eq!(report["data"]["requests"].as_array().unwrap().len(), 2);
    assert_eq!(report["data"]["owned_ranges"].as_array().unwrap().len(), 4);
    assert_eq!(report["data"]["output_written"], false);
    assert!(!output.exists());
    assert_eq!(fs::read(&input).unwrap(), encrypted);

    let duplicate = binary()
        .args([
            "set-many",
            input.to_str().unwrap(),
            "--set",
            "essences.aogami_type_1.owned=1",
            "--set",
            "essences.aogami_type_1.owned=0",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(duplicate.status.code(), Some(2));
    assert!(!output.exists());
}

#[test]
fn encrypted_currency_batch_writes_exact_u32_values() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("input.sav");
    let output = directory.path().join("output.sav");
    let plaintext = support::synthetic::valid_pc_profile();
    let encrypted = crypto::encrypt(&plaintext).unwrap();
    fs::write(&input, &encrypted).unwrap();

    let result = binary()
        .args([
            "set-many",
            input.to_str().unwrap(),
            "--set",
            "game.macca=8207492",
            "--set",
            "game.glory=72",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(result.status.success());
    let decrypted = crypto::decrypt(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(
        u32::from_le_bytes(decrypted[0x3d32..0x3d36].try_into().unwrap()),
        8_207_492
    );
    assert_eq!(
        u32::from_le_bytes(decrypted[0x3d4a..0x3d4e].try_into().unwrap()),
        72
    );
    assert_eq!(fs::read(&input).unwrap(), encrypted);
}

#[test]
fn encrypted_play_time_set_updates_both_copies() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("input.sav");
    let output = directory.path().join("output.sav");
    let plaintext = support::synthetic::valid_pc_profile();
    let encrypted = crypto::encrypt(&plaintext).unwrap();
    fs::write(&input, &encrypted).unwrap();

    let result = binary()
        .args([
            "set",
            input.to_str().unwrap(),
            "game.play_time_seconds",
            "36000",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(result.status.success());
    let decrypted = crypto::decrypt(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(
        u32::from_le_bytes(decrypted[0x4fd..0x501].try_into().unwrap()),
        36_000
    );
    assert_eq!(
        u32::from_le_bytes(decrypted[0x5d0..0x5d4].try_into().unwrap()),
        36_000
    );
    assert_eq!(fs::read(&input).unwrap(), encrypted);
}

#[test]
fn encrypted_item_batch_writes_released_amounts() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("input.sav");
    let output = directory.path().join("output.sav");
    let plaintext = support::synthetic::valid_pc_profile();
    let encrypted = crypto::encrypt(&plaintext).unwrap();
    fs::write(&input, &encrypted).unwrap();

    let result = binary()
        .args([
            "set-many",
            input.to_str().unwrap(),
            "--set",
            "items.life_stone.amount=25",
            "--set",
            "items.chakra_drop.amount=15",
            "--set",
            "items.medicine.amount=30",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(result.status.success());
    let decrypted = crypto::decrypt(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(decrypted[0x4c73], 25);
    assert_eq!(decrypted[0x4c74], 15);
    assert_eq!(decrypted[0x4c7d], 30);
    assert_eq!(fs::read(&input).unwrap(), encrypted);
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
