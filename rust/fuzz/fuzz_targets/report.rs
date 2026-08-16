#![no_main]

use kadishutu::{
    format::detect::{CheckStatus, InputKind, validate_bytes},
    mutation::{ChangedRange, MutationRequest, MutationValidation, OwnedRange},
    report::{MutationData, Report, ValidateData},
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let validation = validate_bytes(data);
    let report = Report::success("validate", ValidateData::from(&validation), Vec::new());
    let _ = serde_json::to_vec(&report);
    let mutation = MutationData {
        input_path: String::from_utf8_lossy(data).into_owned(),
        output_path: "output.sav".to_owned(),
        profile: "smtvv-pc-gamesave-449680".to_owned(),
        request: MutationRequest {
            field: "internal.fuzz".to_owned(),
            value: data.len().to_string(),
        },
        input_kind: InputKind::Decrypted,
        output_kind: InputKind::Decrypted,
        owned_ranges: vec![OwnedRange { start: 64, end: 65 }],
        changed_ranges: vec![ChangedRange { start: 64, end: 65 }],
        sha1_changed: !data.is_empty(),
        pre_validation: MutationValidation {
            length: CheckStatus::Pass,
            gvas: CheckStatus::Pass,
            sha1: CheckStatus::Pass,
            structure: CheckStatus::Pass,
        },
        post_validation: MutationValidation {
            length: CheckStatus::Pass,
            gvas: CheckStatus::Pass,
            sha1: CheckStatus::Pass,
            structure: CheckStatus::Pass,
        },
        dry_run: data.first().is_some_and(|byte| byte & 1 != 0),
        output_written: data.first().is_none_or(|byte| byte & 1 == 0),
    };
    let report = Report::success("set", mutation, Vec::new());
    let _ = serde_json::to_vec(&report);
});
