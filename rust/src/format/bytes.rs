use std::ops::Range;

use super::FormatError;

#[derive(Clone, Copy)]
pub struct ByteView<'a> {
    bytes: &'a [u8],
}

impl<'a> ByteView<'a> {
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn range(&self, offset: usize, length: usize) -> Result<&'a [u8], FormatError> {
        let end = offset
            .checked_add(length)
            .ok_or(FormatError::Arithmetic { offset, length })?;
        self.bytes.get(offset..end).ok_or(FormatError::Bounds {
            offset,
            end,
            length: self.bytes.len(),
        })
    }

    pub fn range_bounds(&self, range: Range<usize>) -> Result<&'a [u8], FormatError> {
        let length = range
            .end
            .checked_sub(range.start)
            .ok_or(FormatError::Arithmetic {
                offset: range.start,
                length: range.end,
            })?;
        self.range(range.start, length)
    }

    pub fn u8(&self, offset: usize) -> Result<u8, FormatError> {
        self.range(offset, 1)?
            .first()
            .copied()
            .ok_or(FormatError::Bounds {
                offset,
                end: offset.saturating_add(1),
                length: self.bytes.len(),
            })
    }

    pub fn u16_le(&self, offset: usize) -> Result<u16, FormatError> {
        Ok(u16::from_le_bytes(self.fixed(offset)?))
    }

    pub fn i16_le(&self, offset: usize) -> Result<i16, FormatError> {
        Ok(i16::from_le_bytes(self.fixed(offset)?))
    }

    pub fn u32_le(&self, offset: usize) -> Result<u32, FormatError> {
        Ok(u32::from_le_bytes(self.fixed(offset)?))
    }

    pub fn u64_le(&self, offset: usize) -> Result<u64, FormatError> {
        Ok(u64::from_le_bytes(self.fixed(offset)?))
    }

    pub fn f32_le(&self, offset: usize) -> Result<f32, FormatError> {
        Ok(f32::from_le_bytes(self.fixed(offset)?))
    }

    pub fn bit(&self, offset: usize, bit: u8) -> Result<bool, FormatError> {
        if bit >= 8 {
            return Err(FormatError::BitIndex(bit));
        }
        Ok(self.u8(offset)? & (1_u8 << bit) != 0)
    }

    pub fn fixed<const N: usize>(&self, offset: usize) -> Result<[u8; N], FormatError> {
        self.range(offset, N)?
            .try_into()
            .map_err(|_| FormatError::Bounds {
                offset,
                end: offset.saturating_add(N),
                length: self.bytes.len(),
            })
    }

    pub fn fstring(&self, offset: usize) -> Result<(String, usize), FormatError> {
        let length = i32::from_le_bytes(self.fixed(offset)?);
        let payload_offset = offset
            .checked_add(4)
            .ok_or(FormatError::Arithmetic { offset, length: 4 })?;
        if length == 0 {
            return Ok((String::new(), payload_offset));
        }
        if length > 0 {
            let byte_length = usize::try_from(length).map_err(|_| FormatError::Arithmetic {
                offset: payload_offset,
                length: usize::MAX,
            })?;
            let payload = self.range(payload_offset, byte_length)?;
            if payload.last().copied() != Some(0) {
                return Err(FormatError::Terminator { offset });
            }
            let text = std::str::from_utf8(&payload[..payload.len() - 1])
                .map_err(|_| FormatError::Utf8 { offset })?;
            let end = payload_offset
                .checked_add(byte_length)
                .ok_or(FormatError::Arithmetic {
                    offset: payload_offset,
                    length: byte_length,
                })?;
            return Ok((text.to_owned(), end));
        }
        let units = length
            .checked_abs()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(FormatError::Arithmetic {
                offset: payload_offset,
                length: usize::MAX,
            })?;
        let byte_length = units.checked_mul(2).ok_or(FormatError::Arithmetic {
            offset: payload_offset,
            length: units,
        })?;
        let payload = self.range(payload_offset, byte_length)?;
        if payload.get(byte_length.saturating_sub(2)..) != Some(&[0, 0][..]) {
            return Err(FormatError::Terminator { offset });
        }
        let text_units: Result<Vec<u16>, FormatError> = payload[..byte_length - 2]
            .chunks_exact(2)
            .map(|unit| {
                let array: [u8; 2] = unit.try_into().map_err(|_| FormatError::Utf16 { offset })?;
                Ok(u16::from_le_bytes(array))
            })
            .collect();
        let text = String::from_utf16(&text_units?).map_err(|_| FormatError::Utf16 { offset })?;
        let end = payload_offset
            .checked_add(byte_length)
            .ok_or(FormatError::Arithmetic {
                offset: payload_offset,
                length: byte_length,
            })?;
        Ok((text, end))
    }
}
