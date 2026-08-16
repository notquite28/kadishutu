use std::{collections::BTreeMap, sync::LazyLock};

use serde::{Deserialize, Serialize};

use super::{
    FormatError,
    detect::{self, CheckStatus, FormatProfile, InputKind, ValidationReport},
};

const LAYOUT_JSON: &str = include_str!("../../docs/evidence/save-layout.v1.json");
const READER_IDS: &[&str] = &[
    "header.custom_version_count",
    "header.custom_version_format",
    "header.engine_version",
    "header.package_file_version",
    "header.save_game_class_name",
    "header.save_game_version",
    "save.file_length",
    "save.gvas_valid",
    "save.input_kind",
    "save.profile",
    "save.sha1_valid",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceState {
    ConfirmedRead,
    ConfirmedWrite,
    Candidate,
    Experimental,
    Unknown,
    Rejected,
}

impl std::fmt::Display for EvidenceState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ConfirmedRead => "confirmed-read",
            Self::ConfirmedWrite => "confirmed-write",
            Self::Candidate => "candidate",
            Self::Experimental => "experimental",
            Self::Unknown => "unknown",
            Self::Rejected => "rejected",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct FieldId(String);

impl FieldId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldDescriptor {
    pub id: String,
    pub read_state: EvidenceState,
    pub write_state: EvidenceState,
    pub sensitive: bool,
}

impl FieldDescriptor {
    #[must_use]
    pub const fn readable(&self) -> bool {
        matches!(
            self.read_state,
            EvidenceState::ConfirmedRead | EvidenceState::ConfirmedWrite
        )
    }

    #[must_use]
    pub fn field_id(&self) -> FieldId {
        FieldId(self.id.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum FieldValue {
    Boolean(bool),
    Integer(u64),
    Text(String),
}

#[derive(Debug)]
pub struct EvidenceCatalog {
    fields: Vec<FieldDescriptor>,
    by_id: BTreeMap<String, usize>,
}

#[derive(Deserialize)]
struct LayoutDocument {
    fields: Vec<FieldDescriptor>,
}

impl EvidenceCatalog {
    fn parse() -> Result<Self, String> {
        let document: LayoutDocument = serde_json::from_str(LAYOUT_JSON)
            .map_err(|error| format!("embedded save layout is invalid: {error}"))?;
        let mut by_id = BTreeMap::new();
        for (index, descriptor) in document.fields.iter().enumerate() {
            if by_id.insert(descriptor.id.clone(), index).is_some() {
                return Err(format!("duplicate embedded field id: {}", descriptor.id));
            }
        }
        for reader_id in READER_IDS {
            let descriptor = by_id
                .get(*reader_id)
                .and_then(|index| document.fields.get(*index))
                .ok_or_else(|| format!("reader has no evidence record: {reader_id}"))?;
            if !descriptor.readable() || descriptor.sensitive {
                return Err(format!(
                    "reader and evidence state disagree for {reader_id}"
                ));
            }
        }
        for descriptor in &document.fields {
            if descriptor.readable() && !READER_IDS.contains(&descriptor.id.as_str()) {
                return Err(format!(
                    "confirmed field has no hard-coded reader: {}",
                    descriptor.id
                ));
            }
        }
        Ok(Self {
            fields: document.fields,
            by_id,
        })
    }

    pub fn get(&self, id: &str) -> Option<&FieldDescriptor> {
        self.by_id.get(id).and_then(|index| self.fields.get(*index))
    }

    pub fn fields(&self) -> impl Iterator<Item = &FieldDescriptor> {
        self.fields.iter()
    }
}

static CATALOG: LazyLock<Result<EvidenceCatalog, String>> = LazyLock::new(EvidenceCatalog::parse);

pub fn evidence_catalog() -> Result<&'static EvidenceCatalog, FormatError> {
    CATALOG
        .as_ref()
        .map_err(|message| FormatError::Structure(message.clone()))
}

pub struct SaveDocument {
    bytes: Vec<u8>,
    validation: ValidationReport,
}

impl SaveDocument {
    pub fn open(bytes: Vec<u8>) -> Result<Self, FormatError> {
        let validation = detect::validate_bytes(&bytes);
        if !validation.is_valid() || validation.input_kind != InputKind::Decrypted {
            return Err(FormatError::UnsupportedProfile);
        }
        Ok(Self { bytes, validation })
    }

    #[must_use]
    pub fn profile(&self) -> Option<FormatProfile> {
        self.validation.profile
    }

    pub fn read(&self, id: &str) -> Result<FieldValue, FormatError> {
        let descriptor = evidence_catalog()?
            .get(id)
            .ok_or_else(|| FormatError::Structure(format!("unknown field id: {id}")))?;
        if !descriptor.readable() || !READER_IDS.contains(&id) {
            return Err(FormatError::Structure(format!(
                "field is not readable: {id}"
            )));
        }
        let header = self.validation.header.as_ref();
        match id {
            "save.file_length" => Ok(FieldValue::Integer(self.bytes.len() as u64)),
            "save.input_kind" => Ok(FieldValue::Text(
                match self.validation.input_kind {
                    InputKind::Decrypted => "decrypted",
                    InputKind::Encrypted => "encrypted",
                    InputKind::Unrecognized => "unrecognized",
                }
                .to_owned(),
            )),
            "save.profile" => self
                .profile()
                .map(|profile| FieldValue::Text(profile.to_string()))
                .ok_or(FormatError::UnsupportedProfile),
            "save.gvas_valid" => Ok(FieldValue::Boolean(
                self.validation.gvas == CheckStatus::Pass,
            )),
            "save.sha1_valid" => Ok(FieldValue::Boolean(
                self.validation.sha1 == CheckStatus::Pass,
            )),
            "header.save_game_version" => header
                .map(|value| FieldValue::Integer(u64::from(value.save_game_version)))
                .ok_or_else(|| FormatError::Structure("header was not parsed".to_owned())),
            "header.package_file_version" => header
                .map(|value| FieldValue::Integer(u64::from(value.package_file_version)))
                .ok_or_else(|| FormatError::Structure("header was not parsed".to_owned())),
            "header.engine_version" => header
                .map(|value| FieldValue::Text(value.engine_version.to_string()))
                .ok_or_else(|| FormatError::Structure("header was not parsed".to_owned())),
            "header.custom_version_format" => header
                .map(|value| FieldValue::Integer(u64::from(value.custom_version_format)))
                .ok_or_else(|| FormatError::Structure("header was not parsed".to_owned())),
            "header.custom_version_count" => header
                .map(|value| FieldValue::Integer(value.custom_versions.len() as u64))
                .ok_or_else(|| FormatError::Structure("header was not parsed".to_owned())),
            "header.save_game_class_name" => header
                .map(|value| FieldValue::Text(value.save_game_class_name.clone()))
                .ok_or_else(|| FormatError::Structure("header was not parsed".to_owned())),
            _ => Err(FormatError::Structure(format!(
                "reader map is incomplete for {id}"
            ))),
        }
    }
}
