//! The CLI commands. Each returns what to print and how to exit; nothing here
//! calls `process::exit`.

mod import_source;
use import_source::read_conversion_source;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use paintcraft_core::types::json::Value;
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba};
use paintcraft_document::{Document, POLICY, hex, import_image, new_document};
use paintcraft_editor::session::Session;
use paintcraft_formats::image::{self, ImageFormat};
use paintcraft_formats::{export, pcnd};
use sha2::{Digest, Sha256};

use crate::report::{envelope, error_value, json_line};
use paintcraft_formats::save::{self, Context, Outcome};

const INPUT_MAX: u64 = 1 << 30;
const OPERATIONS_MAX: u64 = 64 << 20;
const BATCH_SCHEMA: &str = include_str!("../../schemas/batch.schema.json");
const BATCH_V2_SCHEMA: &str = include_str!("../../schemas/batch-v2.schema.json");

pub struct Reply {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub exit: u8,
}

/// The result of a command that writes a file.
struct Written {
    engine: Option<Value>,
    receipt: Value,
    ok: bool,
    summary: String,
}

fn storage(what: &str, e: &std::io::Error) -> Error {
    Error::new(ErrorCode::StorageFailed, format!("{what}: {}", e.kind()))
}

/// Acquire one immutable snapshot of the input and its digest.
fn read_bounded(path: &Path, limit: u64, what: &str) -> Result<(Vec<u8>, String)> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|f| f.take(limit + 1).read_to_end(&mut bytes))
        .map_err(|e| storage(&format!("cannot read {what}"), &e))?;
    if bytes.len() as u64 > limit {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            format!("{what} exceeds the {limit}-byte limit"),
        ));
    }
    let sha = hex(&Sha256::digest(&bytes));
    Ok((bytes, sha))
}

/// Preserve bounded stream inputs while retaining identities for regular files.
/// A classified profile may still require the returned captured identity.
fn read_regular_or_bounded(
    path: &Path,
    limit: u64,
    what: &str,
) -> Result<(
    Vec<u8>,
    String,
    Option<paintcraft_platform::source::SourceIdentity>,
)> {
    if path.metadata().map_err(|e| storage(what, &e))?.is_file() {
        let (bytes, sha, identity) = read_captured(path, limit, what)?;
        Ok((bytes, sha, Some(identity)))
    } else {
        let (bytes, sha) = read_bounded(path, limit, what)?;
        Ok((bytes, sha, None))
    }
}

/// Acquire a regular native resource and retain the same opened identity used
/// for decoding. Scoped users can keep this across conversion and output guards.
fn read_captured(
    path: &Path,
    limit: u64,
    what: &str,
) -> Result<(Vec<u8>, String, paintcraft_platform::source::SourceIdentity)> {
    if !path.metadata().map_err(|e| storage(what, &e))?.is_file() {
        return Err(Error::new(
            ErrorCode::StorageFailed,
            "resource source must be a regular file",
        ));
    }
    let identity = paintcraft_platform::source::SourceIdentity::from_file(
        File::open(path).map_err(|e| storage(what, &e))?,
    )?;
    let metadata = identity.file().metadata().map_err(|e| storage(what, &e))?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "resource source exceeds input allowance",
        ));
    }
    let mut bytes = Vec::new();
    identity
        .file()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| storage(what, &e))?;
    if bytes.len() as u64 > limit {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "resource source exceeds input allowance",
        ));
    }
    let sha = hex(&Sha256::digest(&bytes));
    Ok((bytes, sha, identity))
}

/// `--expect-input-sha256` guards the exact acquired bytes before any parsing.
fn guard(expected: Option<&str>, actual: &str) -> Result<()> {
    guard_named(expected, actual, "--expect-input-sha256")
}
fn guard_named(expected: Option<&str>, actual: &str, flag: &str) -> Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    if expected.len() != 64
        || !expected
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            format!("{flag} must be 64 lowercase hex digits"),
        ));
    }
    if expected != actual {
        return Err(Error::new(
            ErrorCode::ConflictStaleBase,
            format!("input bytes differ from {flag}"),
        ));
    }
    Ok(())
}

fn same_file(a: &Path, b: &Path) -> bool {
    if matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y) {
        return true;
    }
    // The identity library opens both paths. Never open a FIFO or device merely
    // to compare it: that can consume an input handshake or block a command.
    matches!((a.metadata(), b.metadata()), (Ok(x), Ok(y)) if x.is_file() && y.is_file())
        && same_file::is_same_file(a, b).unwrap_or(false)
}

fn finish(json: bool, result: Result<Written>) -> Reply {
    match result {
        Err(e) => {
            let exit = e.code.exit_code();
            let stdout = if json {
                json_line(&envelope(
                    Some(error_value(&e)),
                    None,
                    "engine-rejected",
                    exit,
                ))
            } else {
                Vec::new()
            };
            Reply {
                stdout,
                stderr: format!("error: {e}\n"),
                exit,
            }
        }
        Ok(w) => {
            let exit = if w.ok {
                0
            } else {
                ErrorCode::StorageFailed.exit_code()
            };
            let outcome = if w.ok { "success" } else { "file-incomplete" };
            let stdout = if json {
                json_line(&envelope(w.engine, Some(w.receipt), outcome, exit))
            } else {
                format!("{}\n", w.summary).into_bytes()
            };
            Reply {
                stdout,
                stderr: if w.ok {
                    String::new()
                } else {
                    format!("error: {}\n", w.summary)
                },
                exit,
            }
        }
    }
}

fn bare(json: bool, result: Result<(Value, String)>) -> Reply {
    match result {
        Ok((value, text)) => Reply {
            stdout: if json {
                json_line(&value)
            } else {
                format!("{text}\n").into_bytes()
            },
            stderr: String::new(),
            exit: 0,
        },
        Err(e) => Reply {
            stdout: if json {
                json_line(&error_value(&e))
            } else {
                Vec::new()
            },
            stderr: format!("error: {e}\n"),
            exit: e.code.exit_code(),
        },
    }
}

/// Write `bytes` (or record why they could not be produced) and build the receipt.
fn write_output(
    dest: &Path,
    overwrite: bool,
    bytes: Result<Vec<u8>>,
    ctx: &Context<'_>,
) -> (Value, bool, String) {
    let outcome: Outcome = match bytes {
        Ok(b) => save::save(dest, &b, overwrite),
        Err(e) => save::not_attempted(&e),
    };
    finish_output(dest, ctx, outcome)
}

mod native_save;
use native_save::{write_native, write_native_guarded};

fn finish_output(dest: &Path, ctx: &Context<'_>, outcome: Outcome) -> (Value, bool, String) {
    let summary = if outcome.is_success() {
        format!("wrote {}", dest.display())
    } else {
        format!(
            "{} was not saved safely: {}",
            dest.display(),
            outcome.reasons.join("; ")
        )
    };
    (outcome.receipt(ctx), outcome.is_success(), summary)
}

#[derive(clap::Args, Debug)]
struct Output {
    #[arg(short = 'o', long, value_name = "FILE")]
    output: PathBuf,
    #[arg(long)]
    overwrite: bool,
}

#[derive(clap::Args, Debug)]
struct LossOptions {
    #[arg(long, value_parser = ["error"])]
    on_loss: Option<String>,
    #[arg(long, value_name = "CODES")]
    allow_loss: Option<String>,
}
impl LossOptions {
    fn allowed(&self) -> Result<Vec<String>> {
        self.allowed_for(&export::KNOWN_LOSSES)
    }

    fn allowed_for(&self, known: &[&str]) -> Result<Vec<String>> {
        if self.on_loss.as_deref().is_some_and(|v| v != "error") {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "--on-loss supports only `error`; acknowledge a loss with --allow-loss",
            ));
        }
        let allowed: Vec<String> = self
            .allow_loss
            .as_deref()
            .map(|v| v.split(',').map(str::to_owned).collect())
            .unwrap_or_default();
        if let Some(code) = allowed.iter().find(|code| !known.contains(&code.as_str())) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                format!("unknown loss code {code:?}; known: {}", known.join(", ")),
            ));
        }
        Ok(allowed)
    }
}
pub mod apply;
pub mod capabilities;
mod dds_export;
mod dds_options;
pub mod export_dcx;
pub mod export_icns;
pub mod export_icons;
pub mod export_layered;
pub mod float_job;
pub mod float_options;
pub mod format_profile;
pub mod import;
pub mod import_float;
pub mod inspect;
pub mod inspect_float;
pub mod inspect_icns;
mod msp_export;
mod msp_options;
pub mod new;
pub mod palette;
pub mod paste_image;
pub mod pcx_options;
pub mod render;
pub mod schema;
pub mod svg_options;

pub mod mcp;

pub mod flatten;

pub mod shapes;

pub mod brushes;

pub mod codec_import;
pub mod codec_worker;

pub mod export_jxr;
mod jxr_export_job;
