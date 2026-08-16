use std::ops::Range;

use sha1::{Digest, Sha1};

use crate::format::{FormatError, bytes::ByteView};

pub const HASH_RANGE: Range<usize> = 0..0x14;
pub const HASHED_RANGE_START: usize = 0x40;

pub fn calculate_sha1(bytes: &[u8]) -> Result<[u8; 20], FormatError> {
    let view = ByteView::new(bytes);
    let covered = view.range(
        HASHED_RANGE_START,
        bytes
            .len()
            .checked_sub(HASHED_RANGE_START)
            .ok_or(FormatError::Bounds {
                offset: HASHED_RANGE_START,
                end: HASHED_RANGE_START,
                length: bytes.len(),
            })?,
    )?;
    let digest = Sha1::digest(covered);
    Ok(digest.into())
}
pub fn update_sha1(bytes: &mut [u8]) -> Result<[u8; 20], FormatError> {
    let digest = calculate_sha1(bytes)?;
    let length = bytes.len();
    let stored = bytes.get_mut(HASH_RANGE).ok_or(FormatError::Bounds {
        offset: HASH_RANGE.start,
        end: HASH_RANGE.end,
        length,
    })?;
    stored.copy_from_slice(&digest);
    Ok(digest)
}

pub fn validate_sha1(bytes: &[u8]) -> Result<bool, FormatError> {
    let stored: [u8; 20] = ByteView::new(bytes).fixed(HASH_RANGE.start)?;
    Ok(stored == calculate_sha1(bytes)?)
}
