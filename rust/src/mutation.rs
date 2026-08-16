use std::ops::Range;

use serde::Serialize;
use sha1::{Digest, Sha1};

use crate::{
    crypto,
    error::CliError,
    format::{
        detect::{CheckStatus, FormatProfile, InputKind, ValidationReport, validate_bytes},
        game_save::{EvidenceState, evidence_catalog},
    },
    integrity::{self, HASH_RANGE, HASHED_RANGE_START},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MutationRequest {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OwnedRange {
    pub start: usize,
    pub end: usize,
}

impl OwnedRange {
    fn as_range(self) -> Range<usize> {
        self.start..self.end
    }

    fn contains(self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangedRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MutationValidation {
    pub length: CheckStatus,
    pub gvas: CheckStatus,
    pub sha1: CheckStatus,
    pub structure: CheckStatus,
}

impl From<&ValidationReport> for MutationValidation {
    fn from(report: &ValidationReport) -> Self {
        Self {
            length: report.length,
            gvas: report.gvas,
            sha1: report.sha1,
            structure: report.structure,
        }
    }
}

#[derive(Debug)]
pub struct MutationOutput {
    pub bytes: Vec<u8>,
    pub request: MutationRequest,
    pub profile: FormatProfile,
    pub input_kind: InputKind,
    pub output_kind: InputKind,
    pub owned_ranges: Vec<OwnedRange>,
    pub changed_ranges: Vec<ChangedRange>,
    pub sha1_changed: bool,
    pub pre_validation: MutationValidation,
    pub post_validation: MutationValidation,
}

#[derive(Debug, Clone)]
struct PlannedWrite {
    range: OwnedRange,
    old: Vec<u8>,
    new: Vec<u8>,
}

#[derive(Debug, Clone)]
struct MutationPlan {
    request: MutationRequest,
    writes: Vec<PlannedWrite>,
    old_sha1: [u8; 20],
    expected_sha1: [u8; 20],
}

#[derive(Clone, Copy)]
struct MutationSpec {
    field: &'static str,
    range: OwnedRange,
    encode: fn(&str) -> Result<Vec<u8>, CliError>,
}

pub fn execute_set(source: Vec<u8>, request: MutationRequest) -> Result<MutationOutput, CliError> {
    let prepared = prepare_input(source)?;
    let catalog = evidence_catalog().map_err(|error| CliError::Internal(error.to_string()))?;
    let descriptor = catalog
        .get(&request.field)
        .ok_or_else(|| CliError::CliValue(format!("unknown field id: {}", request.field)))?;
    if descriptor.write_state != EvidenceState::ConfirmedWrite {
        return Err(CliError::CliValue(format!(
            "field is not confirmed-write: {}",
            request.field
        )));
    }
    let spec = mutation_spec(&request.field).ok_or_else(|| {
        CliError::Internal(format!(
            "confirmed-write field has no mutation operation: {}",
            request.field
        ))
    })?;
    execute_prepared(prepared, request, spec, None)
}
fn mutation_spec(_field: &str) -> Option<MutationSpec> {
    None
}

struct PreparedInput {
    plaintext: Vec<u8>,
    input_kind: InputKind,
    validation: ValidationReport,
    profile: FormatProfile,
}

fn prepare_input(source: Vec<u8>) -> Result<PreparedInput, CliError> {
    let source_validation = validate_bytes(&source);
    if !source_validation.is_valid() {
        return Err(validation_error(&source_validation));
    }
    let input_kind = source_validation.input_kind;
    let plaintext = match input_kind {
        InputKind::Decrypted => source,
        InputKind::Encrypted => crypto::decrypt(&source)
            .map_err(|error| CliError::UnsupportedFormat(error.to_string()))?,
        InputKind::Unrecognized => {
            return Err(CliError::UnsupportedFormat(
                "input is not an evidence-approved save".to_owned(),
            ));
        }
    };
    let validation = validate_bytes(&plaintext);
    if !validation.is_valid() || validation.input_kind != InputKind::Decrypted {
        return Err(validation_error(&validation));
    }
    let profile = validation
        .profile
        .ok_or_else(|| CliError::Internal("valid input has no profile".to_owned()))?;
    Ok(PreparedInput {
        plaintext,
        input_kind,
        validation,
        profile,
    })
}

#[cfg(test)]
fn execute_with_spec(
    source: Vec<u8>,
    request: MutationRequest,
    spec: MutationSpec,
    injected_change: Option<(usize, u8)>,
) -> Result<MutationOutput, CliError> {
    let prepared = prepare_input(source)?;
    execute_prepared(prepared, request, spec, injected_change)
}

fn execute_prepared(
    prepared: PreparedInput,
    request: MutationRequest,
    spec: MutationSpec,
    injected_change: Option<(usize, u8)>,
) -> Result<MutationOutput, CliError> {
    if request.field != spec.field {
        return Err(CliError::CliValue(format!(
            "mutation operation does not own field: {}",
            request.field
        )));
    }
    let PreparedInput {
        plaintext,
        input_kind,
        validation: pre_validation,
        profile,
    } = prepared;
    let plan = build_plan(&plaintext, request, spec)?;
    let mut working = plaintext.clone();
    apply_plan(&mut working, &plan)?;
    if let Some((offset, value)) = injected_change {
        let length = working.len();
        let byte = working.get_mut(offset).ok_or_else(|| {
            CliError::Invariant(format!(
                "injected byte offset {offset} is outside buffer length {length}"
            ))
        })?;
        *byte = value;
    }
    let actual_sha1 = integrity::update_sha1(&mut working)
        .map_err(|error| CliError::Invariant(error.to_string()))?;
    let changed_ranges = enforce_diff(&plaintext, &working, &plan)?;
    if actual_sha1 != plan.expected_sha1 {
        return Err(CliError::Invariant(
            "applied mutation produced an unexpected SHA-1".to_owned(),
        ));
    }
    let post_validation = validate_bytes(&working);
    if !post_validation.is_valid() || post_validation.input_kind != InputKind::Decrypted {
        return Err(CliError::Invariant(
            "proposed output failed complete post-mutation validation".to_owned(),
        ));
    }

    let output_kind = input_kind;
    let bytes = match output_kind {
        InputKind::Decrypted => working,
        InputKind::Encrypted => {
            let encrypted = crypto::encrypt(&working)
                .map_err(|error| CliError::Invariant(error.to_string()))?;
            let validation = validate_bytes(&encrypted);
            if !validation.is_valid() || validation.input_kind != InputKind::Encrypted {
                return Err(CliError::Invariant(
                    "encrypted proposed output failed complete validation".to_owned(),
                ));
            }
            encrypted
        }
        InputKind::Unrecognized => {
            return Err(CliError::Internal(
                "validated mutation input became unrecognized".to_owned(),
            ));
        }
    };

    Ok(MutationOutput {
        bytes,
        request: plan.request,
        profile,
        input_kind,
        output_kind,
        owned_ranges: plan.writes.iter().map(|write| write.range).collect(),
        changed_ranges,
        sha1_changed: plan.old_sha1 != plan.expected_sha1,
        pre_validation: MutationValidation::from(&pre_validation),
        post_validation: MutationValidation::from(&post_validation),
    })
}

fn build_plan(
    plaintext: &[u8],
    request: MutationRequest,
    spec: MutationSpec,
) -> Result<MutationPlan, CliError> {
    validate_owned_range(spec.range, plaintext.len())?;
    let new = (spec.encode)(&request.value)?;
    let range = spec.range.as_range();
    if new.len() != range.len() {
        return Err(CliError::Internal(format!(
            "mutation encoder returned {} bytes for owned range {}..{}",
            new.len(),
            spec.range.start,
            spec.range.end
        )));
    }
    let old = plaintext
        .get(range.clone())
        .ok_or_else(|| CliError::Invariant("owned range is outside the save".to_owned()))?
        .to_vec();
    let old_sha1: [u8; 20] = plaintext
        .get(HASH_RANGE)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| CliError::Invariant("SHA-1 field is outside the save".to_owned()))?;
    let write = PlannedWrite {
        range: spec.range,
        old,
        new,
    };
    let expected_sha1 = calculate_planned_sha1(plaintext, std::slice::from_ref(&write))?;
    Ok(MutationPlan {
        request,
        writes: vec![write],
        old_sha1,
        expected_sha1,
    })
}

fn validate_owned_range(range: OwnedRange, length: usize) -> Result<(), CliError> {
    if range.start >= range.end || range.end > length {
        return Err(CliError::Invariant(format!(
            "invalid owned range {}..{} for buffer length {length}",
            range.start, range.end
        )));
    }
    if range.start < HASHED_RANGE_START {
        return Err(CliError::Invariant(format!(
            "mutation range {}..{} is outside the SHA-1-covered payload",
            range.start, range.end
        )));
    }
    Ok(())
}

fn calculate_planned_sha1(plaintext: &[u8], writes: &[PlannedWrite]) -> Result<[u8; 20], CliError> {
    let mut hasher = Sha1::new();
    let mut cursor = HASHED_RANGE_START;
    for write in writes {
        validate_owned_range(write.range, plaintext.len())?;
        if write.range.start < cursor {
            return Err(CliError::Invariant(
                "mutation ranges overlap or are not ordered".to_owned(),
            ));
        }
        hasher.update(
            plaintext
                .get(cursor..write.range.start)
                .ok_or_else(|| CliError::Invariant("planned hash range is invalid".to_owned()))?,
        );
        hasher.update(&write.new);
        cursor = write.range.end;
    }
    hasher.update(
        plaintext
            .get(cursor..)
            .ok_or_else(|| CliError::Invariant("planned hash tail is invalid".to_owned()))?,
    );
    Ok(hasher.finalize().into())
}

fn apply_plan(working: &mut [u8], plan: &MutationPlan) -> Result<(), CliError> {
    for write in &plan.writes {
        let target = working
            .get_mut(write.range.as_range())
            .ok_or_else(|| CliError::Invariant("planned write is outside the save".to_owned()))?;
        if target != write.old {
            return Err(CliError::Invariant(
                "source bytes changed after mutation planning".to_owned(),
            ));
        }
        target.copy_from_slice(&write.new);
    }
    Ok(())
}

fn enforce_diff(
    before: &[u8],
    after: &[u8],
    plan: &MutationPlan,
) -> Result<Vec<ChangedRange>, CliError> {
    let owned = plan
        .writes
        .iter()
        .map(|write| write.range)
        .collect::<Vec<_>>();
    verify_diff_ownership(before, after, &owned)
}

pub fn verify_diff_ownership(
    before: &[u8],
    after: &[u8],
    owned_ranges: &[OwnedRange],
) -> Result<Vec<ChangedRange>, CliError> {
    if before.len() != after.len() {
        return Err(CliError::Invariant(
            "mutation changed the decrypted save length".to_owned(),
        ));
    }
    let mut previous_end = HASHED_RANGE_START;
    for range in owned_ranges {
        validate_owned_range(*range, before.len())?;
        if range.start < previous_end {
            return Err(CliError::Invariant(
                "mutation ranges overlap or are not ordered".to_owned(),
            ));
        }
        previous_end = range.end;
    }
    let mut ranges = Vec::new();
    let mut start = None;
    for (offset, (old, new)) in before.iter().zip(after).enumerate() {
        if old == new {
            if let Some(range_start) = start.take() {
                ranges.push(ChangedRange {
                    start: range_start,
                    end: offset,
                });
            }
            continue;
        }
        let owned =
            HASH_RANGE.contains(&offset) || owned_ranges.iter().any(|range| range.contains(offset));
        if !owned {
            return Err(CliError::Invariant(format!(
                "mutation changed undeclared decrypted byte at offset {offset:#x}"
            )));
        }
        start.get_or_insert(offset);
    }
    if let Some(range_start) = start {
        ranges.push(ChangedRange {
            start: range_start,
            end: after.len(),
        });
    }
    Ok(ranges)
}

fn validation_error(report: &ValidationReport) -> CliError {
    if report.input_kind == InputKind::Unrecognized || report.gvas != CheckStatus::Pass {
        return CliError::UnsupportedFormat(
            "input is not an evidence-approved decrypted or encrypted save".to_owned(),
        );
    }
    if report.length != CheckStatus::Pass || report.structure != CheckStatus::Pass {
        return CliError::Structure("decrypted data failed exact structural validation".to_owned());
    }
    if report.sha1 != CheckStatus::Pass {
        return CliError::Integrity("decrypted data has an invalid SHA-1".to_owned());
    }
    CliError::UnsupportedFormat("input has no evidence-approved profile".to_owned())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use tempfile::tempdir;

    mod synthetic {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/synthetic.rs"
        ));
    }

    const TEST_RANGE: OwnedRange = OwnedRange {
        start: 449_676,
        end: 449_680,
    };
    const TEST_SPEC: MutationSpec = MutationSpec {
        field: "internal.synthetic_u32",
        range: TEST_RANGE,
        encode: encode_u32,
    };

    fn encode_u32(value: &str) -> Result<Vec<u8>, CliError> {
        value
            .parse::<u32>()
            .map(u32::to_le_bytes)
            .map(|bytes| bytes.to_vec())
            .map_err(|_| CliError::CliValue("value must be an unsigned 32-bit integer".to_owned()))
    }

    fn request(value: u32) -> MutationRequest {
        MutationRequest {
            field: TEST_SPEC.field.to_owned(),
            value: value.to_string(),
        }
    }

    #[test]
    fn synthetic_transaction_updates_only_owned_bytes_and_sha1() {
        let source = synthetic::valid_pc_profile();
        let result = execute_with_spec(source.clone(), request(42), TEST_SPEC, None).unwrap();
        assert_eq!(result.input_kind, InputKind::Decrypted);
        assert_eq!(result.output_kind, InputKind::Decrypted);
        assert_eq!(&result.bytes[TEST_RANGE.as_range()], &42_u32.to_le_bytes());
        assert!(result.sha1_changed);
        assert!(integrity::validate_sha1(&result.bytes).unwrap());
        assert_eq!(
            source[0x14..TEST_RANGE.start],
            result.bytes[0x14..TEST_RANGE.start]
        );
    }

    #[test]
    fn planning_does_not_mutate_source_and_repeated_set_is_byte_identical() {
        let source = synthetic::valid_pc_profile();
        let plan = build_plan(&source, request(0), TEST_SPEC).unwrap();
        assert_eq!(source, synthetic::valid_pc_profile());
        assert_eq!(plan.old_sha1, plan.expected_sha1);
        let first = execute_with_spec(source, request(7), TEST_SPEC, None).unwrap();
        let second = execute_with_spec(first.bytes.clone(), request(7), TEST_SPEC, None).unwrap();
        assert_eq!(second.bytes, first.bytes);
        assert!(second.changed_ranges.is_empty());
        assert!(!second.sha1_changed);
    }

    #[test]
    fn synthetic_transaction_persists_without_changing_source() {
        let directory = tempdir().unwrap();
        let input = directory.path().join("input.sav");
        let output = directory.path().join("output.sav");
        let source = synthetic::valid_pc_profile();
        fs::write(&input, &source).unwrap();
        let result = execute_with_spec(source.clone(), request(12), TEST_SPEC, None).unwrap();

        crate::io::write_output(input.as_os_str(), output.as_os_str(), &result.bytes).unwrap();

        assert_eq!(fs::read(&input).unwrap(), source);
        assert_eq!(fs::read(&output).unwrap(), result.bytes);
        let collision =
            crate::io::write_output(input.as_os_str(), output.as_os_str(), b"replacement")
                .unwrap_err();
        assert_eq!(collision.exit_code(), 7);
        assert_eq!(fs::read(&input).unwrap(), source);
    }

    #[test]
    fn undeclared_change_and_invalid_post_state_are_rejected() {
        let source = synthetic::valid_pc_profile();
        let error =
            execute_with_spec(source.clone(), request(3), TEST_SPEC, Some((0x500, 1))).unwrap_err();
        assert_eq!(error.exit_code(), 6);
        assert!(error.to_string().contains("undeclared"));

        let marker_spec = MutationSpec {
            field: TEST_SPEC.field,
            range: OwnedRange {
                start: 0x40,
                end: 0x44,
            },
            encode: |_| Ok(b"NOPE".to_vec()),
        };
        let error = execute_with_spec(source.clone(), request(3), marker_spec, None).unwrap_err();
        assert_eq!(error.exit_code(), 6);
        assert!(error.to_string().contains("post-mutation validation"));

        let overlap = verify_diff_ownership(
            &source,
            &source,
            &[
                OwnedRange {
                    start: 0x100,
                    end: 0x104,
                },
                OwnedRange {
                    start: 0x102,
                    end: 0x106,
                },
            ],
        )
        .unwrap_err();
        assert_eq!(overlap.exit_code(), 6);
    }

    #[test]
    fn encrypted_transaction_preserves_the_envelope() {
        let plaintext = synthetic::valid_pc_profile();
        let encrypted = crypto::encrypt(&plaintext).unwrap();
        let result = execute_with_spec(encrypted, request(99), TEST_SPEC, None).unwrap();
        assert_eq!(result.input_kind, InputKind::Encrypted);
        assert_eq!(result.output_kind, InputKind::Encrypted);
        let validation = validate_bytes(&result.bytes);
        assert!(validation.is_valid());
        assert_eq!(validation.input_kind, InputKind::Encrypted);
    }
}
