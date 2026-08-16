use std::{collections::BTreeSet, env, fs, path::PathBuf, process::Command};

use kadishutu::{
    crypto,
    format::detect::{InputKind, validate_bytes},
};
use serde_json::Value;

#[test]
#[ignore = "requires KADISHUTU_CORPUS_ROOT and private saves"]
fn private_corpus_meets_release_gate() {
    let root = PathBuf::from(
        env::var_os("KADISHUTU_CORPUS_ROOT")
            .expect("KADISHUTU_CORPUS_ROOT is required for the ignored corpus gate"),
    );
    assert!(root.is_dir(), "private corpus root does not exist");
    let status = Command::new("python")
        .args([
            "tools/evidence.py",
            "verify",
            "--root",
            root.to_str().expect("corpus root must be UTF-8"),
            "--manifest",
            "docs/evidence/corpus-manifest.v1.json",
        ])
        .status()
        .expect("run evidence verifier");
    assert!(status.success(), "public manifest verification failed");

    let mut pc_groups = BTreeSet::new();
    let mut cases = 0_usize;
    for entry in fs::read_dir(&root).expect("read corpus root") {
        let path = entry.expect("read corpus entry").path();
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".metadata.json"))
        {
            continue;
        }
        let metadata: Value =
            serde_json::from_slice(&fs::read(&path).expect("read sidecar")).expect("parse sidecar");
        let group = metadata["source_group"]
            .as_str()
            .expect("source_group must be a string")
            .to_owned();
        match metadata["platform"].as_str() {
            Some("pc") => {
                pc_groups.insert(group);
            }
            Some("switch") => {}
            _ => panic!("unsupported sidecar platform"),
        }
        let encrypted = root.join(
            metadata["encrypted_file"]
                .as_str()
                .expect("encrypted_file must be a string"),
        );
        let decrypted = root.join(
            metadata["decrypted_file"]
                .as_str()
                .expect("decrypted_file must be a string"),
        );
        let encrypted_bytes = fs::read(encrypted).expect("read encrypted corpus member");
        let decrypted_bytes = fs::read(decrypted).expect("read decrypted corpus member");
        let encrypted_report = validate_bytes(&encrypted_bytes);
        assert_eq!(
            encrypted_report.input_kind,
            InputKind::Encrypted,
            "encrypted corpus member was not proved"
        );
        assert!(encrypted_report.is_valid());
        let rust_plaintext = crypto::decrypt(&encrypted_bytes).expect("decrypt corpus member");
        assert!(
            rust_plaintext == decrypted_bytes,
            "Rust decryption differs from private companion"
        );
        let report = validate_bytes(&rust_plaintext);
        assert!(
            report.is_valid(),
            "corpus member has no approved valid profile"
        );
        let rust_ciphertext = crypto::encrypt(&rust_plaintext).expect("encrypt corpus member");
        assert!(
            rust_ciphertext == encrypted_bytes,
            "Rust re-encryption differs from original ciphertext"
        );
        cases += 1;
    }
    assert!(
        cases >= 2,
        "at least two complete private PC cases are required"
    );
    assert!(
        pc_groups.len() >= 2,
        "two independent PC source groups are required"
    );
}
