use std::{collections::BTreeSet, ops::Range};

use serde::Serialize;
use sha1::{Digest, Sha1};

use crate::{
    crypto,
    currency::{self, CurrencyDefinition},
    error::CliError,
    essence::{self, EssenceDefinition},
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
    pub requests: Vec<MutationRequest>,
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
    requests: Vec<MutationRequest>,
    writes: Vec<PlannedWrite>,
    old_sha1: [u8; 20],
    expected_sha1: [u8; 20],
}

#[derive(Clone, Copy)]
struct MutationSpec {
    field: &'static str,
    range: OwnedRange,
    encode: fn(&str) -> Result<Vec<u8>, CliError>,
    linked_range: Option<OwnedRange>,
    linked_encode: Option<fn(u8, &str) -> Result<u8, CliError>>,
}

fn essence_spec(definition: EssenceDefinition) -> MutationSpec {
    let owned = definition.owned_offset();
    let metadata = definition.metadata_offset();
    MutationSpec {
        field: definition.field,
        range: OwnedRange {
            start: owned,
            end: owned + 1,
        },
        encode: encode_owned_bool,
        linked_range: Some(OwnedRange {
            start: metadata,
            end: metadata + 1,
        }),
        linked_encode: Some(encode_essence_metadata),
    }
}

fn currency_spec(definition: CurrencyDefinition) -> MutationSpec {
    MutationSpec {
        field: definition.field,
        range: OwnedRange {
            start: definition.offset,
            end: definition.offset + 4,
        },
        encode: encode_u32,
        linked_range: None,
        linked_encode: None,
    }
}

pub fn execute_set(source: Vec<u8>, request: MutationRequest) -> Result<MutationOutput, CliError> {
    execute_set_many(source, vec![request])
}

pub fn execute_set_many(
    source: Vec<u8>,
    requests: Vec<MutationRequest>,
) -> Result<MutationOutput, CliError> {
    if requests.is_empty() {
        return Err(CliError::CliValue(
            "set-many requires at least one assignment".to_owned(),
        ));
    }
    let mut fields = BTreeSet::new();
    let mut specs = Vec::with_capacity(requests.len());
    for request in &requests {
        if !fields.insert(request.field.as_str()) {
            return Err(CliError::CliValue(format!(
                "duplicate mutation field: {}",
                request.field
            )));
        }
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
        specs.push(mutation_spec(&request.field).ok_or_else(|| {
            CliError::Internal(format!(
                "confirmed-write field has no mutation operation: {}",
                request.field
            ))
        })?);
    }
    let prepared = prepare_input(source)?;
    execute_prepared(prepared, requests, specs, None)
}
fn mutation_spec(field: &str) -> Option<MutationSpec> {
    essence::by_field(field)
        .filter(|definition| definition.released())
        .map(essence_spec)
        .or_else(|| currency::by_field(field).map(currency_spec))
}

fn encode_u32(value: &str) -> Result<Vec<u8>, CliError> {
    value
        .parse::<u32>()
        .map(u32::to_le_bytes)
        .map(|bytes| bytes.to_vec())
        .map_err(|_| CliError::CliValue("value must be an unsigned 32-bit integer".to_owned()))
}

fn encode_owned_bool(value: &str) -> Result<Vec<u8>, CliError> {
    match value {
        "0" => Ok(vec![0]),
        "1" => Ok(vec![1]),
        _ => Err(CliError::CliValue(
            "essence ownership must be 0 (absent) or 1 (owned)".to_owned(),
        )),
    }
}

fn encode_essence_metadata(old: u8, value: &str) -> Result<u8, CliError> {
    const NEW: u8 = 0x02;
    const OWNED: u8 = 0x04;
    const ABSENT: u8 = 0x10;
    match value {
        "0" => Ok(old | NEW | OWNED | ABSENT),
        "1" => Ok((old | NEW | OWNED) & !ABSENT),
        _ => Err(CliError::CliValue(
            "essence ownership must be 0 (absent) or 1 (owned)".to_owned(),
        )),
    }
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
    execute_prepared(prepared, vec![request], vec![spec], injected_change)
}

fn execute_prepared(
    prepared: PreparedInput,
    requests: Vec<MutationRequest>,
    specs: Vec<MutationSpec>,
    injected_change: Option<(usize, u8)>,
) -> Result<MutationOutput, CliError> {
    if requests.len() != specs.len() {
        return Err(CliError::Internal(
            "mutation requests and operations differ in length".to_owned(),
        ));
    }
    for (request, spec) in requests.iter().zip(&specs) {
        if request.field != spec.field {
            return Err(CliError::CliValue(format!(
                "mutation operation does not own field: {}",
                request.field
            )));
        }
    }
    let PreparedInput {
        plaintext,
        input_kind,
        validation: pre_validation,
        profile,
    } = prepared;
    let plan = build_batch_plan(&plaintext, requests, specs)?;
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
        requests: plan.requests,
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

#[cfg(test)]
fn build_plan(
    plaintext: &[u8],
    request: MutationRequest,
    spec: MutationSpec,
) -> Result<MutationPlan, CliError> {
    build_batch_plan(plaintext, vec![request], vec![spec])
}

fn build_batch_plan(
    plaintext: &[u8],
    requests: Vec<MutationRequest>,
    specs: Vec<MutationSpec>,
) -> Result<MutationPlan, CliError> {
    let mut writes = Vec::with_capacity(specs.len() * 2);
    for (request, spec) in requests.iter().zip(specs) {
        writes.extend(build_writes(plaintext, request, spec)?);
    }
    writes.sort_by_key(|write| write.range.start);
    if writes
        .windows(2)
        .any(|pair| pair[1].range.start < pair[0].range.end)
    {
        return Err(CliError::Invariant(
            "batch mutation ranges overlap".to_owned(),
        ));
    }
    let old_sha1: [u8; 20] = plaintext
        .get(HASH_RANGE)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| CliError::Invariant("SHA-1 field is outside the save".to_owned()))?;
    let expected_sha1 = calculate_planned_sha1(plaintext, &writes)?;
    Ok(MutationPlan {
        requests,
        writes,
        old_sha1,
        expected_sha1,
    })
}

fn build_writes(
    plaintext: &[u8],
    request: &MutationRequest,
    spec: MutationSpec,
) -> Result<Vec<PlannedWrite>, CliError> {
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
        .get(range)
        .ok_or_else(|| CliError::Invariant("owned range is outside the save".to_owned()))?
        .to_vec();
    let mut writes = vec![PlannedWrite {
        range: spec.range,
        old,
        new,
    }];
    match (spec.linked_range, spec.linked_encode) {
        (Some(linked_range), Some(linked_encode)) => {
            validate_owned_range(linked_range, plaintext.len())?;
            if linked_range.start < spec.range.end || linked_range.end - linked_range.start != 1 {
                return Err(CliError::Internal(
                    "linked mutation range must be one ordered byte".to_owned(),
                ));
            }
            let old = *plaintext.get(linked_range.start).ok_or_else(|| {
                CliError::Invariant("linked owned range is outside the save".to_owned())
            })?;
            let new = linked_encode(old, &request.value)?;
            writes.push(PlannedWrite {
                range: linked_range,
                old: vec![old],
                new: vec![new],
            });
        }
        (None, None) => {}
        _ => {
            return Err(CliError::Internal(
                "linked mutation range and encoder must be declared together".to_owned(),
            ));
        }
    }
    Ok(writes)
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
        linked_range: None,
        linked_encode: None,
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

    fn essence_request(spec: MutationSpec, value: &str) -> MutationRequest {
        MutationRequest {
            field: spec.field.to_owned(),
            value: value.to_owned(),
        }
    }

    fn absent_essence_source(spec: MutationSpec) -> Vec<u8> {
        absent_essences_source(&[spec])
    }

    fn absent_essences_source(specs: &[MutationSpec]) -> Vec<u8> {
        let mut source = synthetic::valid_pc_profile();
        for spec in specs {
            source[spec.range.start] = 0;
            source[spec.linked_range.unwrap().start] = 0x16;
        }
        synthetic::update_hash(&mut source);
        source
    }

    #[test]
    fn linked_essence_registry_matches_legacy_give_and_take_behavior() {
        for definition in essence::released() {
            let spec = essence_spec(definition);
            let source = absent_essence_source(spec);
            let metadata_range = spec.linked_range.unwrap();
            let owned = execute_set(source.clone(), essence_request(spec, "1"))
                .expect("0-to-1 must succeed");
            assert_eq!(owned.bytes[spec.range.start], 1, "{}", spec.field);
            assert_eq!(owned.bytes[metadata_range.start], 0x06, "{}", spec.field);
            assert_eq!(owned.owned_ranges, vec![spec.range, metadata_range]);
            assert!(integrity::validate_sha1(&owned.bytes).unwrap());
            for (offset, (before, after)) in source.iter().zip(&owned.bytes).enumerate() {
                if before != after {
                    assert!(
                        HASH_RANGE.contains(&offset)
                            || spec.range.contains(offset)
                            || metadata_range.contains(offset),
                        "{} changed unexpected byte at {offset:#x}",
                        spec.field
                    );
                }
            }

            let repeated = execute_set(owned.bytes.clone(), essence_request(spec, "1"))
                .expect("idempotent set must succeed");
            assert_eq!(repeated.bytes, owned.bytes, "{}", spec.field);
            assert!(repeated.changed_ranges.is_empty(), "{}", spec.field);
            assert!(!repeated.sha1_changed, "{}", spec.field);

            let absent =
                execute_set(owned.bytes, essence_request(spec, "0")).expect("1-to-0 must succeed");
            assert_eq!(absent.bytes, source, "{}", spec.field);
        }
    }

    #[test]
    fn encrypted_batch_applies_all_linked_writes_once() {
        let specs = essence::released()
            .take(2)
            .map(essence_spec)
            .collect::<Vec<_>>();
        let plaintext = absent_essences_source(&specs);
        let encrypted = crypto::encrypt(&plaintext).unwrap();
        let requests = specs
            .iter()
            .map(|spec| essence_request(*spec, "1"))
            .collect::<Vec<_>>();
        let output = execute_set_many(encrypted, requests).unwrap();
        assert_eq!(output.requests.len(), 2);
        assert_eq!(output.input_kind, InputKind::Encrypted);
        assert_eq!(output.output_kind, InputKind::Encrypted);
        assert_eq!(output.owned_ranges.len(), 4);
        let decrypted = crypto::decrypt(&output.bytes).unwrap();
        for spec in specs {
            assert_eq!(decrypted[spec.range.start], 1);
            assert_eq!(decrypted[spec.linked_range.unwrap().start], 0x06);
        }
    }

    #[test]
    fn batch_rejects_empty_duplicate_and_overlapping_requests() {
        let source = synthetic::valid_pc_profile();
        let empty = execute_set_many(source.clone(), Vec::new()).unwrap_err();
        assert_eq!(empty.exit_code(), 2);

        let spec = essence_spec(essence::released().next().unwrap());
        let duplicate = execute_set_many(
            absent_essence_source(spec),
            vec![essence_request(spec, "1"), essence_request(spec, "0")],
        )
        .unwrap_err();
        assert_eq!(duplicate.exit_code(), 2);
        assert!(duplicate.to_string().contains("duplicate"));

        let overlap = build_batch_plan(
            &source,
            vec![request(1), request(2)],
            vec![TEST_SPEC, TEST_SPEC],
        )
        .unwrap_err();
        assert_eq!(overlap.exit_code(), 6);
        assert!(overlap.to_string().contains("overlap"));
    }

    #[test]
    fn currency_mutations_cover_u32_boundaries_and_idempotence() {
        for definition in currency::CURRENCIES {
            let spec = currency_spec(*definition);
            for value in [0, 8_207_492, u32::MAX] {
                let source = synthetic::valid_pc_profile();
                let request = MutationRequest {
                    field: definition.field.to_owned(),
                    value: value.to_string(),
                };
                let output = execute_set(source.clone(), request.clone()).unwrap();
                assert_eq!(
                    &output.bytes[definition.offset..definition.offset + 4],
                    &value.to_le_bytes()
                );
                assert_eq!(output.owned_ranges, vec![spec.range]);
                assert!(integrity::validate_sha1(&output.bytes).unwrap());
                let repeated = execute_set(output.bytes.clone(), request).unwrap();
                assert_eq!(repeated.bytes, output.bytes);
                assert!(repeated.changed_ranges.is_empty());
            }
        }
    }

    #[test]
    fn currency_mutations_reject_values_outside_u32() {
        for value in ["", "-1", "4294967296", "one"] {
            let error = execute_set(
                synthetic::valid_pc_profile(),
                MutationRequest {
                    field: "game.macca".to_owned(),
                    value: value.to_owned(),
                },
            )
            .unwrap_err();
            assert_eq!(error.exit_code(), 2);
            assert!(error.to_string().contains("unsigned 32-bit"));
        }
    }

    #[test]
    fn essence_metadata_transition_preserves_unknown_bits() {
        assert_eq!(encode_essence_metadata(0x91, "1").unwrap(), 0x87);
        assert_eq!(encode_essence_metadata(0x81, "0").unwrap(), 0x97);
    }

    #[test]
    fn linked_essence_registry_rejects_non_boolean_values() {
        let spec = essence_spec(essence::released().next().unwrap());
        for value in ["", "-1", "2", "true", "owned"] {
            let error =
                execute_set(absent_essence_source(spec), essence_request(spec, value)).unwrap_err();
            assert_eq!(error.exit_code(), 2);
            assert!(error.to_string().contains("0 (absent) or 1 (owned)"));
        }
    }

    #[test]
    fn linked_essence_encrypted_mutation_preserves_encryption() {
        let spec = essence_spec(essence::released().last().unwrap());
        let plaintext = absent_essence_source(spec);
        let encrypted = crypto::encrypt(&plaintext).unwrap();
        let result = execute_set(encrypted, essence_request(spec, "1")).unwrap();
        assert_eq!(result.input_kind, InputKind::Encrypted);
        assert_eq!(result.output_kind, InputKind::Encrypted);
        let decrypted = crypto::decrypt(&result.bytes).unwrap();
        assert_eq!(decrypted[spec.range.start], 1);
        assert_eq!(decrypted[spec.linked_range.unwrap().start], 0x06);
        assert!(validate_bytes(&result.bytes).is_valid());
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
            linked_range: None,
            linked_encode: None,
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
