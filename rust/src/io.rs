use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use crate::error::CliError;

pub const MAX_INPUT_LENGTH: usize = 449_680;

pub fn read_input(path: &OsStr) -> Result<Vec<u8>, CliError> {
    if path == OsStr::new("-") {
        return read_bounded(io::stdin().lock(), "read standard input");
    }
    let path_ref = Path::new(path);
    let file = File::open(path_ref).map_err(|source| CliError::Io {
        context: format!("open {} read-only", path_ref.display()),
        source,
    })?;
    if let Ok(metadata) = file.metadata() {
        if metadata.len() > MAX_INPUT_LENGTH as u64 {
            return Err(CliError::UnsupportedFormat(format!(
                "input exceeds maximum supported length of {MAX_INPUT_LENGTH} bytes"
            )));
        }
    }
    read_bounded(file, &format!("read {}", path_ref.display()))
}

pub fn validate_output_path(input: &OsStr, output: &OsStr) -> Result<(), CliError> {
    if input == OsStr::new("-") || output == OsStr::new("-") {
        return Err(CliError::OutputPolicy(
            "operation requires file input and output paths".to_owned(),
        ));
    }
    let input_path = Path::new(input);
    let output_path = Path::new(output);
    let canonical_input = fs::canonicalize(input_path).map_err(|source| CliError::Io {
        context: format!("resolve input path {}", input_path.display()),
        source,
    })?;
    let resolved_output = resolve_output_path(output_path)?;
    if canonical_input == resolved_output {
        return Err(CliError::OutputPolicy(
            "input and output paths refer to the same file".to_owned(),
        ));
    }
    match fs::symlink_metadata(output_path) {
        Ok(_) => Err(CliError::OutputPolicy(format!(
            "output already exists: {}",
            output_path.display()
        ))),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(CliError::Io {
            context: format!("inspect output path {}", output_path.display()),
            source,
        }),
    }
}

pub fn write_output(input: &OsStr, output: &OsStr, bytes: &[u8]) -> Result<(), CliError> {
    validate_output_path(input, output)?;
    let output_path = Path::new(output);
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|source| CliError::Io {
        context: format!("create temporary output in {}", parent.display()),
        source,
    })?;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.flush())
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|source| CliError::Io {
            context: format!("write temporary output for {}", output_path.display()),
            source,
        })?;
    match temporary.persist_noclobber(output_path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => Err(
            CliError::OutputPolicy(format!("output already exists: {}", output_path.display())),
        ),
        Err(error) => Err(CliError::Io {
            context: format!("rename temporary output to {}", output_path.display()),
            source: error.error,
        }),
    }
}

fn resolve_output_path(output: &Path) -> Result<PathBuf, CliError> {
    if output.file_name().is_none() {
        return Err(CliError::OutputPolicy(
            "output path must name a file".to_owned(),
        ));
    }
    if fs::symlink_metadata(output).is_ok() {
        if let Ok(canonical_output) = fs::canonicalize(output) {
            return Ok(canonical_output);
        }
    }
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let canonical_parent = fs::canonicalize(parent).map_err(|source| CliError::Io {
        context: format!("resolve output directory {}", parent.display()),
        source,
    })?;
    Ok(canonical_parent.join(
        output
            .file_name()
            .ok_or_else(|| CliError::OutputPolicy("output path must name a file".to_owned()))?,
    ))
}

fn read_bounded(reader: impl Read, context: &str) -> Result<Vec<u8>, CliError> {
    let limit = MAX_INPUT_LENGTH
        .checked_add(1)
        .ok_or_else(|| CliError::Internal("input length limit overflow".to_owned()))?;
    let mut bytes = Vec::with_capacity(limit);
    reader
        .take(limit as u64)
        .read_to_end(&mut bytes)
        .map_err(|source| CliError::Io {
            context: context.to_owned(),
            source,
        })?;
    if bytes.len() > MAX_INPUT_LENGTH {
        return Err(CliError::UnsupportedFormat(format!(
            "input exceeds maximum supported length of {MAX_INPUT_LENGTH} bytes"
        )));
    }
    Ok(bytes)
}
