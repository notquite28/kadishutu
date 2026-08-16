pub mod bytes;
pub mod detect;
pub mod game_save;
pub mod unreal;

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FormatError {
    #[error("byte range {offset}..{end} is outside input length {length}")]
    Bounds {
        offset: usize,
        end: usize,
        length: usize,
    },
    #[error("byte range arithmetic overflow at offset {offset} with length {length}")]
    Arithmetic { offset: usize, length: usize },
    #[error("invalid UTF-8 FString at offset {offset}")]
    Utf8 { offset: usize },
    #[error("invalid UTF-16LE FString at offset {offset}")]
    Utf16 { offset: usize },
    #[error("FString at offset {offset} has no trailing NUL")]
    Terminator { offset: usize },
    #[error("invalid bit index {0}; expected 0..8")]
    BitIndex(u8),
    #[error("invalid Unreal header: {0}")]
    Structure(String),
    #[error("no evidence-approved format profile matches this input")]
    UnsupportedProfile,
}
