use serde::Serialize;

use super::{FormatError, bytes::ByteView};

pub const HEADER_START: usize = 0x40;
pub const HEADER_END: usize = 0x4d0;
pub const CUSTOM_VERSION_ENTRY_SIZE: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub change_list: u32,
    pub branch: String,
}

impl std::fmt::Display for EngineVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}.{}.{}-{}{}",
            self.major, self.minor, self.patch, self.change_list, self.branch
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomVersion {
    pub guid: [u8; 16],
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnrealHeader {
    pub save_game_version: u32,
    pub package_file_version: u32,
    pub engine_version: EngineVersion,
    pub custom_version_format: u32,
    pub custom_versions: Vec<CustomVersion>,
    pub save_game_class_name: String,
    pub end_offset: usize,
}

pub fn parse_header(bytes: &[u8]) -> Result<UnrealHeader, FormatError> {
    let view = ByteView::new(bytes);
    if view.fixed::<4>(HEADER_START)? != *b"GVAS" {
        return Err(FormatError::Structure("missing GVAS marker".to_owned()));
    }
    view.range(HEADER_START, HEADER_END - HEADER_START)?;
    let save_game_version = view.u32_le(0x44)?;
    let package_file_version = view.u32_le(0x48)?;
    let major = view.u16_le(0x4c)?;
    let minor = view.u16_le(0x4e)?;
    let patch = view.u16_le(0x50)?;
    let change_list = view.u32_le(0x52)?;
    let (branch, branch_end) = view.fstring(0x56)?;
    if branch_end > HEADER_END {
        return Err(FormatError::Structure(
            "engine branch exceeds the header boundary".to_owned(),
        ));
    }
    let custom_version_format = view.u32_le(branch_end)?;
    let count_offset = branch_end.checked_add(4).ok_or(FormatError::Arithmetic {
        offset: branch_end,
        length: 4,
    })?;
    let count = usize::try_from(view.u32_le(count_offset)?).map_err(|_| {
        FormatError::Structure("custom-version count is not representable".to_owned())
    })?;
    let entries_offset = count_offset.checked_add(4).ok_or(FormatError::Arithmetic {
        offset: count_offset,
        length: 4,
    })?;
    let entries_length =
        count
            .checked_mul(CUSTOM_VERSION_ENTRY_SIZE)
            .ok_or(FormatError::Arithmetic {
                offset: entries_offset,
                length: count,
            })?;
    let entries_end =
        entries_offset
            .checked_add(entries_length)
            .ok_or(FormatError::Arithmetic {
                offset: entries_offset,
                length: entries_length,
            })?;
    if entries_end > HEADER_END {
        return Err(FormatError::Structure(
            "custom versions exceed the header boundary".to_owned(),
        ));
    }
    let mut custom_versions = Vec::with_capacity(count);
    for index in 0..count {
        let relative =
            index
                .checked_mul(CUSTOM_VERSION_ENTRY_SIZE)
                .ok_or(FormatError::Arithmetic {
                    offset: entries_offset,
                    length: index,
                })?;
        let offset = entries_offset
            .checked_add(relative)
            .ok_or(FormatError::Arithmetic {
                offset: entries_offset,
                length: relative,
            })?;
        custom_versions.push(CustomVersion {
            guid: view.fixed(offset)?,
            version: view.u32_le(
                offset
                    .checked_add(16)
                    .ok_or(FormatError::Arithmetic { offset, length: 16 })?,
            )?,
        });
    }
    let (save_game_class_name, end_offset) = view.fstring(entries_end)?;
    if end_offset > HEADER_END {
        return Err(FormatError::Structure(
            "save-game class exceeds the header boundary".to_owned(),
        ));
    }
    Ok(UnrealHeader {
        save_game_version,
        package_file_version,
        engine_version: EngineVersion {
            major,
            minor,
            patch,
            change_list,
            branch,
        },
        custom_version_format,
        custom_versions,
        save_game_class_name,
        end_offset,
    })
}

pub fn candidate_signature_matches(header: &UnrealHeader) -> bool {
    header.save_game_version == 2
        && header.package_file_version == 522
        && header.engine_version
            == (EngineVersion {
                major: 4,
                minor: 27,
                patch: 2,
                change_list: 0,
                branch: "++UE4+Release-4.27".to_owned(),
            })
        && header.custom_version_format == 3
        && header.custom_versions.len() == 55
        && header.save_game_class_name == "SaveObject"
        && header.end_offset == HEADER_END
}
