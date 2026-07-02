// SPDX-License-Identifier: Apache-2.0

//! Command implementations over `loki-convert` and `loki-print`.

use std::path::Path;
use std::time::Duration;

use loki_convert::{ConvertError, ConvertOptions, Format, PdfProfile, convert};
use loki_print::{ColorMode, Duplex, IppPrinter, PrintError, PrintOptions};

use crate::cli::{Cli, Command, ConvertArgs, PrintArgs};

/// CLI failures (all map to exit code 1).
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("cannot determine the format of {path}: {reason} (use --from/--to)")]
    UnknownFileFormat { path: String, reason: &'static str },
    #[error(transparent)]
    Convert(#[from] ConvertError),
    #[error(transparent)]
    Print(#[from] PrintError),
    #[error("failed to read {path}: {source}")]
    ReadInput {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write {path}: {source}")]
    WriteOutput {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

pub fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Convert(args) | Command::Render(args) => run_convert(&args),
        Command::Print(args) => run_print(&args),
        Command::Formats => {
            for (source, target) in loki_convert::supported_pairs() {
                println!("{source} -> {target}");
            }
            Ok(())
        }
    }
}

fn format_of(path: &Path, explicit: Option<&str>) -> Result<Format, CliError> {
    if let Some(name) = explicit {
        return name.parse().map_err(CliError::Convert);
    }
    let ext =
        path.extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| CliError::UnknownFileFormat {
                path: path.display().to_string(),
                reason: "no file extension",
            })?;
    Format::from_extension(ext).ok_or_else(|| CliError::UnknownFileFormat {
        path: path.display().to_string(),
        reason: "unrecognised extension",
    })
}

fn convert_options(profile: Option<&str>, title: Option<&str>) -> Result<ConvertOptions, CliError> {
    let pdf_profile = match profile {
        Some(profile) => profile.parse::<PdfProfile>().map_err(CliError::Convert)?,
        None => PdfProfile::Default,
    };
    Ok(ConvertOptions {
        pdf_profile,
        title: title.map(str::to_owned),
    })
}

fn run_convert(args: &ConvertArgs) -> Result<(), CliError> {
    let source = format_of(&args.input, args.from.as_deref())?;
    let target = format_of(&args.output, args.to.as_deref())?;
    let options = convert_options(args.profile.as_deref(), args.title.as_deref())?;
    let input = std::fs::read(&args.input).map_err(|e| CliError::ReadInput {
        path: args.input.display().to_string(),
        source: e,
    })?;
    let output = convert(source, &input, target, &options)?;
    for warning in &output.warnings {
        eprintln!("warning: {warning}");
    }
    std::fs::write(&args.output, &output.bytes).map_err(|e| CliError::WriteOutput {
        path: args.output.display().to_string(),
        source: e,
    })?;
    println!(
        "{} -> {} ({} bytes)",
        args.input.display(),
        args.output.display(),
        output.bytes.len()
    );
    Ok(())
}

fn run_print(args: &PrintArgs) -> Result<(), CliError> {
    let source = format_of(&args.input, None)?;
    let input = std::fs::read(&args.input).map_err(|e| CliError::ReadInput {
        path: args.input.display().to_string(),
        source: e,
    })?;
    // Already-rendered PDFs dispatch as-is; anything else renders first
    // (render → PDF → IPP, ADR-C023).
    let pdf = if source == Format::Pdf {
        input
    } else {
        convert(source, &input, Format::Pdf, &ConvertOptions::default())?.bytes
    };

    let file_name = args
        .input
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    let options = PrintOptions {
        copies: args.copies,
        duplex: match (args.duplex, args.short_edge) {
            (false, _) => Duplex::Simplex,
            (true, false) => Duplex::LongEdge,
            (true, true) => Duplex::ShortEdge,
        },
        media: args.media.clone(),
        color: if args.mono {
            ColorMode::Monochrome
        } else {
            ColorMode::Auto
        },
        job_title: args.title.clone().or(file_name),
    };

    let printer = IppPrinter::connect(&args.printer)?;
    let job_id = printer.print_pdf(pdf, &options)?;
    println!("submitted job {job_id} to {}", args.printer);
    if !args.no_wait {
        let state = printer.wait_for_completion(job_id, Duration::from_secs(args.timeout))?;
        println!("job {job_id} finished: {state:?}");
    }
    Ok(())
}
