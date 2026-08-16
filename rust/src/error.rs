use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    CliValue(String),
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
    #[error("{0}")]
    UnsupportedFormat(String),
    #[error("{0}")]
    Integrity(String),
    #[error("{0}")]
    Structure(String),
    #[error("{0}")]
    Invariant(String),
    #[error("{0}")]
    OutputPolicy(String),
    #[error("{0}")]
    Internal(String),
}

impl CliError {
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::CliValue(_) => 2,
            Self::Io { .. } => 3,
            Self::UnsupportedFormat(_) => 4,
            Self::Integrity(_) | Self::Structure(_) => 5,
            Self::Invariant(_) => 6,
            Self::OutputPolicy(_) => 7,
            Self::Internal(_) => 8,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::CliValue(_) => "cli_value",
            Self::Io { .. } => "io",
            Self::UnsupportedFormat(_) => "unsupported_format",
            Self::Integrity(_) => "integrity",
            Self::Structure(_) => "structure",
            Self::Invariant(_) => "invariant",
            Self::OutputPolicy(_) => "output_policy",
            Self::Internal(_) => "internal",
        }
    }
}
