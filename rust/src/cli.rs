use std::{ffi::OsString, io::Write};

use clap::{Parser, Subcommand, ValueEnum, error::ErrorKind};
use serde::Serialize;

use crate::{
    crypto,
    error::CliError,
    format::{
        detect::{CheckStatus, InputKind, ValidationReport, validate_bytes},
        game_save::{SaveDocument, evidence_catalog},
    },
    io::{read_input, validate_output_path, write_output},
    mutation::{MutationRequest, execute_set, execute_set_many},
    report::{
        ConversionData, FieldEntry, GetData, InspectData, MutationBatchData, MutationData, Report,
        ValidateData, Warning, conversion_text, fields_text, mutation_batch_text, mutation_text,
        validate_text,
    },
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Parser)]
#[command(name = "kadishutu", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Validate {
        file: OsString,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    Inspect {
        file: OsString,
        #[arg(long = "field")]
        fields: Vec<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    Get {
        file: OsString,
        field: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    Decrypt {
        input: OsString,
        #[arg(long)]
        output: OsString,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    Encrypt {
        input: OsString,
        #[arg(long)]
        output: OsString,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    Set {
        input: OsString,
        field: String,
        value: String,
        #[arg(long)]
        output: OsString,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    SetMany {
        input: OsString,
        #[arg(long = "set")]
        assignments: Vec<String>,
        #[arg(long)]
        output: OsString,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}

pub fn run() -> Result<(), CliError> {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            error.print().map_err(|source| CliError::Io {
                context: "print command help".to_owned(),
                source,
            })?;
            return Ok(());
        }
        Err(error) => return Err(CliError::CliValue(error.to_string())),
    };
    match cli.command {
        Command::Validate { file, format } => validate(&file, format),
        Command::Inspect {
            file,
            fields,
            format,
        } => inspect(&file, &fields, format),
        Command::Get {
            file,
            field,
            format,
        } => get(&file, &field, format),
        Command::Decrypt {
            input,
            output,
            format,
        } => convert(&input, &output, format, Conversion::Decrypt),
        Command::Encrypt {
            input,
            output,
            format,
        } => convert(&input, &output, format, Conversion::Encrypt),
        Command::Set {
            input,
            field,
            value,
            output,
            dry_run,
            format,
        } => set(&input, &field, &value, &output, dry_run, format),
        Command::SetMany {
            input,
            assignments,
            output,
            dry_run,
            format,
        } => set_many(&input, &assignments, &output, dry_run, format),
    }
}

fn validate(file: &OsString, format: OutputFormat) -> Result<(), CliError> {
    let bytes = match read_input(file) {
        Ok(bytes) => bytes,
        Err(error) => return emit_error("validate", format, &error),
    };
    let validation = validate_bytes(&bytes);
    let data = ValidateData::from(&validation);
    let error = validation_error(&validation);
    match format {
        OutputFormat::Json => {
            let report = Report {
                schema_version: 1,
                command: "validate",
                ok: error.is_none(),
                data: Some(data),
                warnings: Vec::new(),
                error: error.as_ref().map(|value| crate::report::ErrorData {
                    kind: value.kind().to_owned(),
                    message: value.to_string(),
                }),
            };
            emit_json(&report)?;
        }
        OutputFormat::Text => print!("{}", validate_text(&data, error.is_none())),
    }
    error.map_or(Ok(()), Err)
}

fn inspect(file: &OsString, requested: &[String], format: OutputFormat) -> Result<(), CliError> {
    let catalog = match evidence_catalog() {
        Ok(catalog) => catalog,
        Err(error) => {
            let cli_error = CliError::Internal(error.to_string());
            return emit_error("inspect", format, &cli_error);
        }
    };
    let descriptors = if requested.is_empty() {
        catalog
            .fields()
            .filter(|field| field.readable() && !field.sensitive)
            .collect::<Vec<_>>()
    } else {
        let mut result = Vec::with_capacity(requested.len());
        for id in requested {
            let Some(descriptor) = catalog.get(id) else {
                let error = CliError::CliValue(format!("unknown field id: {id}"));
                return emit_error("inspect", format, &error);
            };
            result.push(descriptor);
        }
        result
    };
    let bytes = match read_input(file) {
        Ok(bytes) => bytes,
        Err(error) => return emit_error("inspect", format, &error),
    };
    let document = match SaveDocument::open(bytes) {
        Ok(document) => document,
        Err(error) => {
            let cli_error = CliError::UnsupportedFormat(error.to_string());
            return emit_error("inspect", format, &cli_error);
        }
    };
    let mut entries = Vec::with_capacity(descriptors.len());
    let mut warnings = Vec::new();
    for descriptor in descriptors {
        let value = if descriptor.readable() {
            Some(
                document
                    .read(&descriptor.id)
                    .map_err(|error| CliError::Structure(error.to_string()))?,
            )
        } else {
            warnings.push(Warning {
                code: "field_unavailable".to_owned(),
                message: format!("field is {} and is not readable", descriptor.read_state),
                field: Some(descriptor.id.clone()),
            });
            None
        };
        entries.push(FieldEntry {
            id: descriptor.id.clone(),
            evidence_state: descriptor.read_state,
            readable: descriptor.readable(),
            sensitive: descriptor.sensitive,
            value,
        });
    }
    let data = InspectData {
        profile: document.profile().map(|profile| profile.to_string()),
        fields: entries,
    };
    match format {
        OutputFormat::Json => emit_json(&Report::success("inspect", data, warnings))?,
        OutputFormat::Text => print!("{}", fields_text(&data.fields)),
    }
    Ok(())
}

fn get(file: &OsString, id: &str, format: OutputFormat) -> Result<(), CliError> {
    let catalog = match evidence_catalog() {
        Ok(catalog) => catalog,
        Err(error) => {
            let cli_error = CliError::Internal(error.to_string());
            return emit_error("get", format, &cli_error);
        }
    };
    let Some(descriptor) = catalog.get(id) else {
        let error = CliError::CliValue(format!("unknown field id: {id}"));
        return emit_error("get", format, &error);
    };
    if !descriptor.readable() {
        let error = CliError::CliValue(format!("field is not confirmed-read: {id}"));
        return emit_error("get", format, &error);
    }
    let bytes = match read_input(file) {
        Ok(bytes) => bytes,
        Err(error) => return emit_error("get", format, &error),
    };
    let document = match SaveDocument::open(bytes) {
        Ok(document) => document,
        Err(error) => {
            let cli_error = CliError::UnsupportedFormat(error.to_string());
            return emit_error("get", format, &cli_error);
        }
    };
    let entry = FieldEntry {
        id: descriptor.id.clone(),
        evidence_state: descriptor.read_state,
        readable: true,
        sensitive: descriptor.sensitive,
        value: Some(
            document
                .read(id)
                .map_err(|error| CliError::Structure(error.to_string()))?,
        ),
    };
    let data = GetData { field: entry };
    match format {
        OutputFormat::Json => emit_json(&Report::success("get", data, Vec::new()))?,
        OutputFormat::Text => print!("{}", fields_text(std::slice::from_ref(&data.field))),
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Conversion {
    Decrypt,
    Encrypt,
}

impl Conversion {
    const fn command(self) -> &'static str {
        match self {
            Self::Decrypt => "decrypt",
            Self::Encrypt => "encrypt",
        }
    }
}

fn convert(
    input: &OsString,
    output: &OsString,
    format: OutputFormat,
    conversion: Conversion,
) -> Result<(), CliError> {
    let command = conversion.command();
    let source = match read_input(input) {
        Ok(bytes) => bytes,
        Err(error) => return emit_error(command, format, &error),
    };
    let (converted, profile, input_kind, output_kind) = match conversion {
        Conversion::Decrypt => {
            let plaintext = match crypto::decrypt(&source) {
                Ok(bytes) => bytes,
                Err(error) => {
                    let cli_error = CliError::UnsupportedFormat(error.to_string());
                    return emit_error(command, format, &cli_error);
                }
            };
            let validation = validate_bytes(&plaintext);
            if let Some(error) = validation_error(&validation) {
                return emit_error(command, format, &error);
            }
            let profile = validation
                .profile
                .ok_or_else(|| CliError::Internal("valid input has no profile".to_owned()))?;
            (
                plaintext,
                profile,
                InputKind::Encrypted,
                InputKind::Decrypted,
            )
        }
        Conversion::Encrypt => {
            let validation = validate_bytes(&source);
            if validation.input_kind != InputKind::Decrypted {
                let error = CliError::UnsupportedFormat(
                    "encrypt requires a valid decrypted supported save".to_owned(),
                );
                return emit_error(command, format, &error);
            }
            if let Some(error) = validation_error(&validation) {
                return emit_error(command, format, &error);
            }
            let profile = validation
                .profile
                .ok_or_else(|| CliError::Internal("valid input has no profile".to_owned()))?;
            let ciphertext =
                crypto::encrypt(&source).map_err(|error| CliError::Structure(error.to_string()))?;
            let encrypted_validation = validate_bytes(&ciphertext);
            if !encrypted_validation.is_valid()
                || encrypted_validation.input_kind != InputKind::Encrypted
            {
                let error = CliError::Internal(
                    "encrypted output failed supported-profile validation".to_owned(),
                );
                return emit_error(command, format, &error);
            }
            (
                ciphertext,
                profile,
                InputKind::Decrypted,
                InputKind::Encrypted,
            )
        }
    };
    if let Err(error) = write_output(input, output, &converted) {
        return emit_error(command, format, &error);
    }
    let data = ConversionData {
        input_path: input.to_string_lossy().into_owned(),
        output_path: output.to_string_lossy().into_owned(),
        profile: profile.to_string(),
        input_kind,
        output_kind,
        file_length: converted.len(),
    };
    match format {
        OutputFormat::Json => emit_json(&Report::success(command, data, Vec::new()))?,
        OutputFormat::Text => print!("{}", conversion_text(command, &data)),
    }
    Ok(())
}
fn set(
    input: &OsString,
    field: &str,
    value: &str,
    output: &OsString,
    dry_run: bool,
    format: OutputFormat,
) -> Result<(), CliError> {
    let source = match read_input(input) {
        Ok(bytes) => bytes,
        Err(error) => return emit_error("set", format, &error),
    };
    let mutation = match execute_set(
        source,
        MutationRequest {
            field: field.to_owned(),
            value: value.to_owned(),
        },
    ) {
        Ok(mutation) => mutation,
        Err(error) => return emit_error("set", format, &error),
    };
    let output_written = match persist_set(input, output, &mutation.bytes, dry_run) {
        Ok(written) => written,
        Err(error) => return emit_error("set", format, &error),
    };
    let data = MutationData {
        input_path: input.to_string_lossy().into_owned(),
        output_path: output.to_string_lossy().into_owned(),
        profile: mutation.profile.to_string(),
        request: mutation.requests[0].clone(),
        input_kind: mutation.input_kind,
        output_kind: mutation.output_kind,
        owned_ranges: mutation.owned_ranges,
        changed_ranges: mutation.changed_ranges,
        sha1_changed: mutation.sha1_changed,
        pre_validation: mutation.pre_validation,
        post_validation: mutation.post_validation,
        dry_run,
        output_written,
    };
    match format {
        OutputFormat::Json => emit_json(&Report::success("set", data, Vec::new()))?,
        OutputFormat::Text => print!("{}", mutation_text(&data)),
    }
    Ok(())
}

fn set_many(
    input: &OsString,
    assignments: &[String],
    output: &OsString,
    dry_run: bool,
    format: OutputFormat,
) -> Result<(), CliError> {
    let requests = match parse_assignments(assignments) {
        Ok(requests) => requests,
        Err(error) => return emit_error("set-many", format, &error),
    };
    let source = match read_input(input) {
        Ok(bytes) => bytes,
        Err(error) => return emit_error("set-many", format, &error),
    };
    let mutation = match execute_set_many(source, requests) {
        Ok(mutation) => mutation,
        Err(error) => return emit_error("set-many", format, &error),
    };
    let output_written = match persist_set(input, output, &mutation.bytes, dry_run) {
        Ok(written) => written,
        Err(error) => return emit_error("set-many", format, &error),
    };
    let data = MutationBatchData {
        input_path: input.to_string_lossy().into_owned(),
        output_path: output.to_string_lossy().into_owned(),
        profile: mutation.profile.to_string(),
        requests: mutation.requests,
        input_kind: mutation.input_kind,
        output_kind: mutation.output_kind,
        owned_ranges: mutation.owned_ranges,
        changed_ranges: mutation.changed_ranges,
        sha1_changed: mutation.sha1_changed,
        pre_validation: mutation.pre_validation,
        post_validation: mutation.post_validation,
        dry_run,
        output_written,
    };
    match format {
        OutputFormat::Json => emit_json(&Report::success("set-many", data, Vec::new()))?,
        OutputFormat::Text => print!("{}", mutation_batch_text(&data)),
    }
    Ok(())
}

fn parse_assignments(assignments: &[String]) -> Result<Vec<MutationRequest>, CliError> {
    assignments
        .iter()
        .map(|assignment| {
            let (field, value) = assignment.split_once('=').ok_or_else(|| {
                CliError::CliValue(format!("assignment must use FIELD=VALUE: {assignment}"))
            })?;
            if field.is_empty() || value.is_empty() {
                return Err(CliError::CliValue(format!(
                    "assignment must use non-empty FIELD=VALUE: {assignment}"
                )));
            }
            Ok(MutationRequest {
                field: field.to_owned(),
                value: value.to_owned(),
            })
        })
        .collect()
}
fn persist_set(
    input: &OsString,
    output: &OsString,
    bytes: &[u8],
    dry_run: bool,
) -> Result<bool, CliError> {
    validate_output_path(input, output)?;
    if dry_run {
        return Ok(false);
    }
    write_output(input, output, bytes)?;
    Ok(true)
}

fn validation_error(report: &ValidationReport) -> Option<CliError> {
    if report.gvas != CheckStatus::Pass {
        return Some(CliError::UnsupportedFormat(
            "input is not an evidence-approved decrypted or encrypted save".to_owned(),
        ));
    }
    if report.length != CheckStatus::Pass || report.structure != CheckStatus::Pass {
        return Some(CliError::Structure(
            "decrypted data failed exact structural validation".to_owned(),
        ));
    }
    if report.sha1 != CheckStatus::Pass {
        return Some(CliError::Integrity(
            "decrypted data has an invalid SHA-1".to_owned(),
        ));
    }
    if report.profile.is_none() {
        return Some(CliError::UnsupportedFormat(
            "candidate signature is not an evidence-approved profile".to_owned(),
        ));
    }
    None
}

fn emit_error<T: Serialize>(
    command: &'static str,
    format: OutputFormat,
    error: &CliError,
) -> Result<T, CliError> {
    if matches!(format, OutputFormat::Json) {
        emit_json(&Report::<serde_json::Value>::failure(command, error))?;
    }
    Err(match error {
        CliError::CliValue(message) => CliError::CliValue(message.clone()),
        CliError::Io { context, source } => CliError::Io {
            context: context.clone(),
            source: std::io::Error::new(source.kind(), source.to_string()),
        },
        CliError::UnsupportedFormat(message) => CliError::UnsupportedFormat(message.clone()),
        CliError::Integrity(message) => CliError::Integrity(message.clone()),
        CliError::Structure(message) => CliError::Structure(message.clone()),
        CliError::OutputPolicy(message) => CliError::OutputPolicy(message.clone()),
        CliError::Invariant(message) => CliError::Invariant(message.clone()),
        CliError::Internal(message) => CliError::Internal(message.clone()),
    })
}

fn emit_json(value: &impl Serialize) -> Result<(), CliError> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, value)
        .map_err(|error| CliError::Internal(format!("serialize JSON report: {error}")))?;
    lock.write_all(b"\n").map_err(|source| CliError::Io {
        context: "write standard output".to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn dry_run_validates_paths_without_writing() {
        let directory = tempdir().unwrap();
        let input = directory.path().join("input.sav");
        let output = directory.path().join("output.sav");
        fs::write(&input, b"source").unwrap();

        let written = persist_set(
            &input.as_os_str().to_owned(),
            &output.as_os_str().to_owned(),
            b"proposed",
            true,
        )
        .unwrap();

        assert!(!written);
        assert_eq!(fs::read(input).unwrap(), b"source");
        assert!(!output.exists());
    }
}
