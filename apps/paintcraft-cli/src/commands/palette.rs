//! ASE commands exchange typed resources without creating or editing a document.
use super::*;
use paintcraft_core::types::ids::deterministic_id;
use paintcraft_formats::{
    ase::{self, Entry, Kind, Palette, Swatch},
    registry::{self, Family, Operation, Platform, Request},
};

#[derive(clap::Args, Debug)]
pub struct Options {
    #[command(subcommand)]
    command: Command,
}
#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Inspect palette groups and precise channel values, without RGB conversion.
    Inspect(Input),
    /// Write a fresh ASE copy, preserving admitted palette values and grouping.
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
    let mut request = Request::new(frontend, operation, Family::Ase, Platform::Native);
    request.extension = Some(path.extension().and_then(|s| s.to_str()).unwrap_or(""));
    let decision = registry::admission(request);
    if decision.available() {
        Ok(())
    } else {
        decision.require_allowed(request)
    }
}
fn acquire(input: &Input) -> Result<(Palette, String)> {
    admit(
        &input.input,
        input.profile.frontend.into(),
        Operation::ImportAsset,
    )?;
    if !std::fs::metadata(&input.input)
        .map_err(|e| storage("cannot inspect palette", &e))?
        .is_file()
    {
        return Err(Error::new(
            ErrorCode::StorageFailed,
            "palette input must be a regular file",
        ));
    }
    let file = File::open(&input.input).map_err(|e| storage("cannot read palette", &e))?;
    let metadata = file
        .metadata()
        .map_err(|e| storage("cannot inspect palette", &e))?;
    if !metadata.is_file() {
        return Err(Error::new(
            ErrorCode::StorageFailed,
            "palette input must be a regular file",
        ));
    }
    if metadata.len() > ase::BYTES_MAX as u64 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "ASE source exceeds byte allowance",
        ));
    }
    let mut bytes = Vec::new();
    file.take(ase::BYTES_MAX as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| storage("cannot read palette", &e))?;
    if bytes.len() > ase::BYTES_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "ASE source exceeds byte allowance",
        ));
    }
    let sha = hex(&Sha256::digest(&bytes));
    guard(input.expect_input_sha256.as_deref(), &sha)?;
    Ok((ase::decode(&bytes, &|| Ok(()))?, sha))
}
fn swatch(value: &Swatch) -> Value {
    Value::object([
        ("name", Value::str(&value.name)),
        (
            "kind",
            Value::str(match value.kind {
                Kind::Global => "global",
                Kind::Spot => "spot",
                Kind::Normal => "process",
            }),
        ),
        ("model", Value::str(value.color.model())),
        // The wire JSON contract is integer-only. Hex IEEE754 bits preserve
        // exact finite f32 values, including negative zero, without rounding.
        (
            "channels_f32_be",
            Value::Array(
                value
                    .color
                    .channels()
                    .iter()
                    .map(|v| Value::str(format!("{:08x}", v.to_bits())))
                    .collect(),
            ),
        ),
    ])
}
fn inspection(palette: &Palette, sha: &str) -> Value {
    Value::object([
        ("schema", Value::str("paintcraft.palette-inspection/v1")),
        ("resource_type", Value::str("palette")),
        (
            "resource_id",
            Value::str(deterministic_id(&["palette", "ase", sha]).to_string()),
        ),
        ("source_sha256", Value::str(sha)),
        ("swatches", Value::Int(palette.swatch_count() as i64)),
        (
            "entries",
            Value::Array(
                palette
                    .entries()
                    .iter()
                    .map(|entry| match entry {
                        Entry::Swatch(color) => Value::object([("swatch", swatch(color))]),
                        Entry::Group { name, swatches } => Value::object([
                            ("group", Value::str(name)),
                            (
                                "swatches",
                                Value::Array(swatches.iter().map(swatch).collect()),
                            ),
                        ]),
                    })
                    .collect(),
            ),
        ),
    ])
}
pub fn run(options: Options, json: bool) -> Reply {
    match options.command {
        Command::Inspect(input) => bare(
            json,
            (|| {
                let (palette, sha) = acquire(&input)?;
                let mut lines = vec![format!("ASE palette: {} swatches", palette.swatch_count())];
                for entry in palette.entries() {
                    match entry {
                        Entry::Swatch(color) => {
                            lines.push(format!("{:?} ({})", color.name, color.color.model()))
                        }
                        Entry::Group { name, swatches } => {
                            lines.push(format!("{name:?}:"));
                            lines.extend(swatches.iter().map(|color| {
                                format!("  {:?} ({})", color.name, color.color.model())
                            }));
                        }
                    }
                }
                Ok((inspection(&palette, &sha), lines.join("\n")))
            })(),
        ),
        Command::Export(export) => finish(
            json,
            (|| {
                let frontend = export.input.profile.frontend.into();
                // Output and frontend policy precede source acquisition and codecs.
                admit(&export.output.output, frontend, Operation::ExportAsset)?;
                if same_file(&export.input.input, &export.output.output) {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "palette export must not replace its source",
                    ));
                }
                let (palette, sha) = acquire(&export.input)?;
                let bytes = ase::encode(&palette, &|| Ok(()))?;
                let context = Context {
                    operation: "export-asset",
                    // This receipt records a frozen palette resource, not a document.
                    source_state: deterministic_id(&["palette", "ase", &sha]),
                    source_sha256: Some(sha.clone()),
                    losses_acknowledged: vec![],
                    destination: &export.output.output,
                };
                let outcome = save::save_with(
                    &export.output.output,
                    export.output.overwrite,
                    ase::BYTES_MAX as u64,
                    |sink| {
                        sink.write_all(&bytes)
                            .map_err(|e| storage("cannot stage palette", &e))
                    },
                );
                let (receipt, ok, summary) =
                    finish_output(&export.output.output, &context, outcome);
                Ok(Written {
                    engine: Some(inspection(&palette, &sha)),
                    receipt,
                    ok,
                    summary,
                })
            })(),
        ),
    }
}
