use serde::Serialize;
use sha1::{Digest, Sha1};

use crate::{
    crypto,
    format::{bytes::ByteView, unreal},
    integrity,
    io::MAX_INPUT_LENGTH,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    Decrypted,
    Encrypted,
    Unrecognized,
}

impl std::fmt::Display for InputKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Decrypted => "decrypted",
            Self::Encrypted => "encrypted",
            Self::Unrecognized => "unrecognized",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FormatProfile {
    #[serde(rename = "smtvv-pc-gamesave-449680")]
    SmtVvPcGameSave449680,
}

impl std::fmt::Display for FormatProfile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::SmtVvPcGameSave449680 => "smtvv-pc-gamesave-449680",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Fail,
    NotRun,
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::NotRun => "not_run",
        })
    }
}

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub input_kind: InputKind,
    pub profile: Option<FormatProfile>,
    pub platform_evidence: Vec<&'static str>,
    pub file_length: usize,
    pub length: CheckStatus,
    pub gvas: CheckStatus,
    pub sha1: CheckStatus,
    pub structure: CheckStatus,
    pub header: Option<unreal::UnrealHeader>,
}

impl ValidationReport {
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.profile.is_some()
            && matches!(self.length, CheckStatus::Pass)
            && matches!(self.gvas, CheckStatus::Pass)
            && matches!(self.sha1, CheckStatus::Pass)
            && matches!(self.structure, CheckStatus::Pass)
    }
}

const PC_HEADER_SHA1: [u8; 20] = [
    0x75, 0xbd, 0xaa, 0xce, 0x7d, 0x8e, 0x1b, 0xda, 0x14, 0x8a, 0x79, 0xe8, 0x31, 0xab, 0x2c, 0xb8,
    0x12, 0x83, 0xd8, 0xa4,
];

#[must_use]
pub fn validate_bytes(bytes: &[u8]) -> ValidationReport {
    if has_gvas(bytes) {
        return validate_decrypted(bytes);
    }

    let report = unrecognized_report(bytes);
    if bytes.len() != MAX_INPUT_LENGTH {
        return report;
    }
    let Ok(plaintext) = crypto::decrypt(bytes) else {
        return report;
    };
    let mut decrypted_report = validate_decrypted(&plaintext);
    if !decrypted_report.is_valid() {
        return report;
    }
    decrypted_report.input_kind = InputKind::Encrypted;
    decrypted_report
}

fn has_gvas(bytes: &[u8]) -> bool {
    ByteView::new(bytes)
        .fixed::<4>(0x40)
        .is_ok_and(|marker| marker == *b"GVAS")
}

fn unrecognized_report(bytes: &[u8]) -> ValidationReport {
    ValidationReport {
        input_kind: InputKind::Unrecognized,
        profile: None,
        platform_evidence: Vec::new(),
        file_length: bytes.len(),
        length: if bytes.len() == MAX_INPUT_LENGTH {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        gvas: if bytes.len() > MAX_INPUT_LENGTH {
            CheckStatus::NotRun
        } else {
            CheckStatus::Fail
        },
        sha1: CheckStatus::NotRun,
        structure: CheckStatus::NotRun,
        header: None,
    }
}

fn validate_decrypted(bytes: &[u8]) -> ValidationReport {
    let mut report = unrecognized_report(bytes);
    if bytes.len() > MAX_INPUT_LENGTH {
        return report;
    }
    report.gvas = if has_gvas(bytes) {
        CheckStatus::Pass
    } else {
        CheckStatus::Fail
    };
    if report.gvas != CheckStatus::Pass {
        return report;
    }
    report.input_kind = InputKind::Decrypted;
    if report.length != CheckStatus::Pass {
        report.structure = CheckStatus::Fail;
        return report;
    }
    let view = ByteView::new(bytes);
    match unreal::parse_header(bytes) {
        Ok(header) if unreal::candidate_signature_matches(&header) => {
            let header_bytes = match view.range(
                unreal::HEADER_START,
                unreal::HEADER_END - unreal::HEADER_START,
            ) {
                Ok(header_bytes) => header_bytes,
                Err(_) => {
                    report.structure = CheckStatus::Fail;
                    return report;
                }
            };
            let header_digest: [u8; 20] = Sha1::digest(header_bytes).into();
            if header_digest != PC_HEADER_SHA1 {
                report.structure = CheckStatus::Fail;
                return report;
            }
            report.structure = CheckStatus::Pass;
            report.profile = Some(FormatProfile::SmtVvPcGameSave449680);
            report.platform_evidence = vec!["pc"];
            report.header = Some(header);
        }
        Ok(_) | Err(_) => {
            report.structure = CheckStatus::Fail;
            return report;
        }
    }
    report.sha1 = match integrity::validate_sha1(bytes) {
        Ok(true) => CheckStatus::Pass,
        Ok(false) | Err(_) => CheckStatus::Fail,
    };
    report
}
