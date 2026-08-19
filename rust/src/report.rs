use serde::Serialize;

use crate::{
    error::CliError,
    format::{
        detect::{CheckStatus, InputKind, ValidationReport},
        game_save::{EvidenceState, FieldValue},
    },
    mutation::{ChangedRange, MutationRequest, MutationValidation, OwnedRange},
};

#[derive(Debug, Clone, Serialize)]
pub struct Warning {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorData {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report<T: Serialize> {
    pub schema_version: u8,
    pub command: &'static str,
    pub ok: bool,
    pub data: Option<T>,
    pub warnings: Vec<Warning>,
    pub error: Option<ErrorData>,
}

impl<T: Serialize> Report<T> {
    #[must_use]
    pub fn success(command: &'static str, data: T, warnings: Vec<Warning>) -> Self {
        Self {
            schema_version: 1,
            command,
            ok: true,
            data: Some(data),
            warnings,
            error: None,
        }
    }

    #[must_use]
    pub fn failure(command: &'static str, error: &CliError) -> Self {
        Self {
            schema_version: 1,
            command,
            ok: false,
            data: None,
            warnings: Vec::new(),
            error: Some(ErrorData {
                kind: error.kind().to_owned(),
                message: error.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidateData {
    pub input_kind: InputKind,
    pub profile: Option<String>,
    pub platform_evidence: Vec<String>,
    pub file_length: usize,
    pub length: CheckStatus,
    pub gvas: CheckStatus,
    pub sha1: CheckStatus,
    pub structure: CheckStatus,
}

impl From<&ValidationReport> for ValidateData {
    fn from(report: &ValidationReport) -> Self {
        Self {
            input_kind: report.input_kind,
            profile: report.profile.map(|profile| profile.to_string()),
            platform_evidence: report
                .platform_evidence
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            file_length: report.file_length,
            length: report.length,
            gvas: report.gvas,
            sha1: report.sha1,
            structure: report.structure,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldEntry {
    pub id: String,
    pub evidence_state: EvidenceState,
    pub readable: bool,
    pub sensitive: bool,
    pub value: Option<FieldValue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectData {
    pub profile: Option<String>,
    pub fields: Vec<FieldEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GetData {
    pub field: FieldEntry,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConversionData {
    pub input_path: String,
    pub output_path: String,
    pub profile: String,
    pub input_kind: InputKind,
    pub output_kind: InputKind,
    pub file_length: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct MutationData {
    pub input_path: String,
    pub output_path: String,
    pub profile: String,
    pub request: MutationRequest,
    pub input_kind: InputKind,
    pub output_kind: InputKind,
    pub owned_ranges: Vec<OwnedRange>,
    pub changed_ranges: Vec<ChangedRange>,
    pub sha1_changed: bool,
    pub pre_validation: MutationValidation,
    pub post_validation: MutationValidation,
    pub dry_run: bool,
    pub output_written: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MutationBatchData {
    pub input_path: String,
    pub output_path: String,
    pub profile: String,
    pub requests: Vec<MutationRequest>,
    pub input_kind: InputKind,
    pub output_kind: InputKind,
    pub owned_ranges: Vec<OwnedRange>,
    pub changed_ranges: Vec<ChangedRange>,
    pub sha1_changed: bool,
    pub pre_validation: MutationValidation,
    pub post_validation: MutationValidation,
    pub dry_run: bool,
    pub output_written: bool,
}

pub fn validate_text(data: &ValidateData, ok: bool) -> String {
    let platforms = if data.platform_evidence.is_empty() {
        "none".to_owned()
    } else {
        data.platform_evidence.join(", ")
    };
    format!(
        "Input kind: {}\nProfile: {}\nPlatform evidence: {}\nLength: {}\nGVAS: {}\nSHA-1: {}\nStructure: {}\nResult: {}\n",
        data.input_kind,
        data.profile.as_deref().unwrap_or("none"),
        platforms,
        data.length,
        data.gvas,
        data.sha1,
        data.structure,
        if ok { "valid" } else { "invalid" }
    )
}

pub fn conversion_text(command: &str, data: &ConversionData) -> String {
    format!(
        "Operation: {command}\nInput: {}\nOutput: {}\nProfile: {}\nInput kind: {}\nOutput kind: {}\nFile length: {}\nResult: success\n",
        data.input_path,
        data.output_path,
        data.profile,
        data.input_kind,
        data.output_kind,
        data.file_length,
    )
}
pub fn mutation_text(data: &MutationData) -> String {
    let owned = render_ranges(&data.owned_ranges);
    let changed = data
        .changed_ranges
        .iter()
        .map(|range| format!("{:#x}..{:#x}", range.start, range.end))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Operation: set\nInput: {}\nOutput: {}\nProfile: {}\nField: {}\nRequested value: {}\nInput kind: {}\nOutput kind: {}\nOwned decrypted ranges: {}\nChanged decrypted ranges: {}\nSHA-1 changed: {}\nPre-validation: {}\nPost-validation: {}\nDry run: {}\nOutput written: {}\nResult: success\n",
        data.input_path,
        data.output_path,
        data.profile,
        data.request.field,
        data.request.value,
        data.input_kind,
        data.output_kind,
        owned,
        if changed.is_empty() { "none" } else { &changed },
        data.sha1_changed,
        validation_result(&data.pre_validation),
        validation_result(&data.post_validation),
        data.dry_run,
        data.output_written,
    )
}

pub fn mutation_batch_text(data: &MutationBatchData) -> String {
    let requests = data
        .requests
        .iter()
        .map(|request| format!("{}={}", request.field, request.value))
        .collect::<Vec<_>>()
        .join(", ");
    let owned = render_ranges(&data.owned_ranges);
    let changed = data
        .changed_ranges
        .iter()
        .map(|range| format!("{:#x}..{:#x}", range.start, range.end))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Operation: set-many\nInput: {}\nOutput: {}\nProfile: {}\nRequests: {}\nInput kind: {}\nOutput kind: {}\nOwned ranges: {}\nChanged ranges: {}\nSHA-1 changed: {}\nPre-validation: {}\nPost-validation: {}\nDry run: {}\nOutput written: {}\n",
        data.input_path,
        data.output_path,
        data.profile,
        requests,
        data.input_kind,
        data.output_kind,
        owned,
        if changed.is_empty() { "none" } else { &changed },
        data.sha1_changed,
        validation_result(&data.pre_validation),
        validation_result(&data.post_validation),
        data.dry_run,
        data.output_written,
    )
}

fn render_ranges(ranges: &[OwnedRange]) -> String {
    let rendered = ranges
        .iter()
        .map(|range| format!("{:#x}..{:#x}", range.start, range.end))
        .collect::<Vec<_>>()
        .join(", ");
    if rendered.is_empty() {
        "none".to_owned()
    } else {
        rendered
    }
}

fn validation_result(validation: &MutationValidation) -> &'static str {
    if validation.length == CheckStatus::Pass
        && validation.gvas == CheckStatus::Pass
        && validation.sha1 == CheckStatus::Pass
        && validation.structure == CheckStatus::Pass
    {
        "pass"
    } else {
        "fail"
    }
}

pub fn fields_text(fields: &[FieldEntry]) -> String {
    let mut output = String::new();
    for field in fields {
        let rendered = match &field.value {
            Some(FieldValue::Boolean(value)) => value.to_string(),
            Some(FieldValue::Integer(value)) => value.to_string(),
            Some(FieldValue::Text(value)) => value.clone(),
            Some(FieldValue::Essence(value)) => format!(
                "amount={} metadata={:#04x} fusion_available={} main_menu_present={} new={} owned_flag={} consistent={}",
                value.amount,
                value.metadata,
                value.fusion_available,
                value.main_menu_present,
                value.new,
                value.owned_flag,
                value.consistent
            ),
            None => format!("unavailable ({})", field.evidence_state),
        };
        output.push_str(&field.id);
        output.push_str(": ");
        output.push_str(&rendered);
        output.push('\n');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_state() -> MutationValidation {
        MutationValidation {
            length: CheckStatus::Pass,
            gvas: CheckStatus::Pass,
            sha1: CheckStatus::Pass,
            structure: CheckStatus::Pass,
        }
    }

    #[test]
    fn mutation_report_contains_exact_public_change_data() {
        let data = MutationData {
            input_path: "input.sav".to_owned(),
            output_path: "output.sav".to_owned(),
            profile: "smtvv-pc-gamesave-449680".to_owned(),
            request: MutationRequest {
                field: "internal.synthetic_u32".to_owned(),
                value: "7".to_owned(),
            },
            input_kind: InputKind::Encrypted,
            output_kind: InputKind::Encrypted,
            owned_ranges: vec![OwnedRange {
                start: 449_676,
                end: 449_680,
            }],
            changed_ranges: vec![ChangedRange {
                start: 449_676,
                end: 449_677,
            }],
            sha1_changed: true,
            pre_validation: valid_state(),
            post_validation: valid_state(),
            dry_run: true,
            output_written: false,
        };
        let json = serde_json::to_value(Report::success("set", data, Vec::new())).unwrap();
        assert_eq!(json["command"], "set");
        assert_eq!(json["data"]["changed_ranges"][0]["start"], 449_676);
        assert_eq!(json["data"]["dry_run"], true);
        assert_eq!(json["data"]["output_written"], false);
        assert!(!json.to_string().contains("0123456789abcdef"));
    }
}
