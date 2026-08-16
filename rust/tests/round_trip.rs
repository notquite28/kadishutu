mod support;

use assert_cmd::prelude::*;
use std::{fs, process::Command, thread, time::Duration};

use tempfile::tempdir;

fn binary() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("kadishutu"))
}

#[test]
fn read_only_commands_preserve_content_permissions_and_mtime() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("candidate.sav");
    let bytes = support::synthetic::valid_pc_profile();
    fs::write(&path, &bytes).unwrap();
    let before = fs::metadata(&path).unwrap();
    thread::sleep(Duration::from_millis(20));

    let commands: &[&[&str]] = &[
        &["validate"],
        &["inspect", "--field", "game.macca"],
        &["get", "save.profile"],
    ];
    for arguments in commands {
        let mut command = binary();
        command.arg(arguments[0]).arg(&path);
        command.args(&arguments[1..]);
        let status = command.status().unwrap();
        assert!(status.success());
    }

    let after = fs::metadata(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    assert_eq!(
        before.permissions().readonly(),
        after.permissions().readonly()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(before.permissions().mode(), after.permissions().mode());
    }
}

#[test]
fn conversion_commands_are_exact_and_preserve_both_sources() {
    let directory = tempdir().unwrap();
    let decrypted = directory.path().join("decrypted.sav");
    let encrypted = directory.path().join("encrypted.sav");
    let recovered = directory.path().join("recovered.sav");
    let plaintext = support::synthetic::valid_pc_profile();
    fs::write(&decrypted, &plaintext).unwrap();

    let encrypt_output = binary()
        .args([
            "encrypt",
            decrypted.to_str().unwrap(),
            "--output",
            encrypted.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(encrypt_output.status.success());
    let encrypt_report: serde_json::Value = serde_json::from_slice(&encrypt_output.stdout).unwrap();
    assert_eq!(encrypt_report["command"], "encrypt");
    assert_eq!(encrypt_report["data"]["input_kind"], "decrypted");
    assert_eq!(encrypt_report["data"]["output_kind"], "encrypted");
    assert_eq!(fs::read(&decrypted).unwrap(), plaintext);
    let ciphertext = fs::read(&encrypted).unwrap();

    let decrypt_output = binary()
        .args([
            "decrypt",
            encrypted.to_str().unwrap(),
            "--output",
            recovered.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(decrypt_output.status.success());
    let decrypt_report: serde_json::Value = serde_json::from_slice(&decrypt_output.stdout).unwrap();
    assert_eq!(decrypt_report["command"], "decrypt");
    assert_eq!(decrypt_report["data"]["input_kind"], "encrypted");
    assert_eq!(decrypt_report["data"]["output_kind"], "decrypted");
    assert_eq!(fs::read(&encrypted).unwrap(), ciphertext);
    assert_eq!(fs::read(&recovered).unwrap(), plaintext);
}

#[test]
fn conversion_rejects_collision_alias_and_invalid_input_without_changes() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("input.sav");
    let output = directory.path().join("output.sav");
    let plaintext = support::synthetic::valid_pc_profile();
    fs::write(&input, &plaintext).unwrap();
    fs::write(&output, b"existing").unwrap();

    binary()
        .args([
            "encrypt",
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .code(7);
    assert_eq!(fs::read(&input).unwrap(), plaintext);
    assert_eq!(fs::read(&output).unwrap(), b"existing");

    binary()
        .args([
            "encrypt",
            input.to_str().unwrap(),
            "--output",
            input.to_str().unwrap(),
        ])
        .assert()
        .code(7);
    assert_eq!(fs::read(&input).unwrap(), plaintext);

    let partial = directory.path().join("partial.sav");
    let absent = directory.path().join("absent.sav");
    fs::write(&partial, [0_u8; 15]).unwrap();
    binary()
        .args([
            "decrypt",
            partial.to_str().unwrap(),
            "--output",
            absent.to_str().unwrap(),
        ])
        .assert()
        .code(4);
    assert!(!absent.exists());

    let mut bad_hash = plaintext.clone();
    bad_hash[0] ^= 1;
    fs::write(&input, &bad_hash).unwrap();
    binary()
        .args([
            "encrypt",
            input.to_str().unwrap(),
            "--output",
            absent.to_str().unwrap(),
        ])
        .assert()
        .code(5);
    assert_eq!(fs::read(&input).unwrap(), bad_hash);
    assert!(!absent.exists());
}

#[cfg(unix)]
#[test]
fn failed_atomic_rename_removes_temporary_output() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("input.sav");
    fs::write(&input, support::synthetic::valid_pc_profile()).unwrap();
    let output = directory.path().join("x".repeat(300));

    binary()
        .args([
            "encrypt",
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .code(3);

    let entries = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries, [input.file_name().unwrap()]);
}
