//! CSH shape libraries are resources, never image documents.
mod schema;
use super::*;
use paintcraft_core::types::ids::deterministic_id;
use paintcraft_formats::{
    csh::{self, Library},
    registry::{self, Family, Operation, Platform, Request},
};

#[derive(clap::Args, Debug)]
pub struct Options {
    #[command(subcommand)]
    command: Command,
}
#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Inspect names, bounds and ordered records with exact signed normalized coordinates.
    Inspect(Input),
    /// Write a fresh CSH copy, preserving names, bounds, topology and all record flags.
    Export(Export),
}
#[derive(clap::Args, Debug)]
struct Input {
    input: PathBuf,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[command(flatten)]
    profile: format_profile::Options,
}
#[derive(clap::Args, Debug)]
struct Export {
    #[command(flatten)]
    input: Input,
    #[command(flatten)]
    output: Output,
}
fn admit(path: &Path, frontend: registry::Frontend, operation: Operation) -> Result<()> {
    let mut request = Request::new(frontend, operation, Family::Csh, Platform::Native);
    request.extension = Some(path.extension().and_then(|s| s.to_str()).unwrap_or(""));
    let decision = registry::admission(request);
    if decision.available() {
        Ok(())
    } else {
        decision.require_allowed(request)
    }
}
pub(super) fn acquire(
    path: &Path,
    expected: Option<&str>,
    frontend: registry::Frontend,
) -> Result<(Library, String)> {
    admit(path, frontend, Operation::ImportAsset)?;
    if !std::fs::metadata(path)
        .map_err(|e| storage("cannot inspect shape", &e))?
        .is_file()
    {
        return Err(Error::new(
            ErrorCode::StorageFailed,
            "shape input must be a regular file",
        ));
    }
    let file = File::open(path).map_err(|e| storage("cannot read shape", &e))?;
    let metadata = file
        .metadata()
        .map_err(|e| storage("cannot inspect shape", &e))?;
    if !metadata.is_file() {
        return Err(Error::new(
            ErrorCode::StorageFailed,
            "shape input must be a regular file",
        ));
    }
    if metadata.len() > csh::BYTES_MAX as u64 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "CSH source exceeds byte allowance",
        ));
    }
    let mut bytes = Vec::new();
    file.take(csh::BYTES_MAX as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| storage("cannot read shape", &e))?;
    if bytes.len() > csh::BYTES_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "CSH source exceeds byte allowance",
        ));
    }
    let sha = hex(&Sha256::digest(&bytes));
    guard(expected, &sha)?;
    Ok((csh::decode(&bytes, &|| Ok(()))?, sha))
}
pub(super) fn inspection(library: &Library, sha: &str) -> Result<Value> {
    schema::inspection(library, sha)
}
pub(super) fn schema() -> Result<Value> {
    schema::schema()
}
pub fn run(options: Options, json: bool) -> Reply {
    match options.command {
        Command::Inspect(input) => bare(
            json,
            (|| {
                let (shape, sha) = acquire(
                    &input.input,
                    input.expect_input_sha256.as_deref(),
                    input.profile.frontend.into(),
                )?;
                let lines =
                    std::iter::once(format!("CSH library: {} shapes", shape.shapes().len()))
                        .chain(shape.shapes().iter().map(|shape| {
                            format!("{:?}: {} records", shape.name, shape.records.len())
                        }))
                        .collect::<Vec<_>>()
                        .join("\n");
                Ok((inspection(&shape, &sha)?, lines))
            })(),
        ),
        Command::Export(export) => finish(
            json,
            export_copy(
                &export.input.input,
                &export.output.output,
                export.output.overwrite,
                export.input.expect_input_sha256.as_deref(),
                export.input.profile.frontend.into(),
            ),
        ),
    }
}

pub(super) fn export_copy(
    input: &Path,
    output: &Path,
    overwrite: bool,
    expected: Option<&str>,
    frontend: registry::Frontend,
) -> Result<Written> {
    // Output and frontend policy precede source acquisition and codecs.
    admit(output, frontend, Operation::ExportAsset)?;
    if same_file(input, output) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "shape export must not replace its source",
        ));
    }
    let (shape, sha) = acquire(input, expected, frontend)?;
    let bytes = csh::encode(&shape, &|| Ok(()))?;
    let context = Context {
        operation: "export-asset",
        // This receipt records a frozen shape resource, not a document.
        source_state: deterministic_id(&["shape-library", "csh", &sha]),
        source_sha256: Some(sha.clone()),
        losses_acknowledged: vec![],
        destination: output,
    };
    let outcome = save::save_with(output, overwrite, csh::BYTES_MAX as u64, |sink| {
        sink.write_all(&bytes)
            .map_err(|e| storage("cannot stage shape", &e))
    });
    let (receipt, ok, summary) = finish_output(output, &context, outcome);
    Ok(Written {
        engine: Some(inspection(&shape, &sha)?),
        receipt,
        ok,
        summary,
    })
}
