//! Qualified ABR assets, kept separate from image/document import.
mod painting;
mod schema;
use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_formats::{
    abr::{
        self,
        adoption::{self, AdoptedPreset, RequiredConversion, StaticTipPolicy},
    },
    registry::{self, Family, Operation, Platform, Request},
};
pub(super) use painting::{Stroke, paint};
use std::sync::Arc;
pub(super) const BUDGET_BYTES: u64 = 128 << 20;

#[derive(clap::Args, Debug)]
pub struct Options {
    #[command(subcommand)]
    command: Command,
}
#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Inspect ordered preset capabilities without creating/changing a document.
    Inspect(Input),
    /// Paint with a qualified R8 native static tip; full Photoshop dynamics are refused.
    Paint(Paint),
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
struct Paint {
    project: PathBuf,
    #[arg(long)]
    brush: PathBuf,
    #[arg(long)]
    preset: usize,
    /// Explicit renderer policy; this does not emulate Photoshop dynamics.
    #[arg(long,value_parser=["native-grid-v1"])]
    policy: String,
    /// JSON array of integer [x,y] pairs; positions use point-scale units per pixel.
    #[arg(long)]
    points: PathBuf,
    #[arg(long)]
    color: String,
    #[arg(long, default_value_t = 1)]
    point_scale: u32,
    #[arg(long)]
    layer_id: Option<String>,
    #[arg(long)]
    allow_spacing_quantization: bool,
    #[arg(long)]
    expect_brush_sha256: Option<String>,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[command(flatten)]
    profile: format_profile::Options,
    #[command(flatten)]
    output: Output,
}
// The source identity is captured from the same opened file used for decoding,
// not a later pathname lookup. Existing same-file handles supply OS identity.
pub(super) struct Loaded {
    pub source: Arc<abr::Library>,
    pub sha: String,
    pub path: PathBuf,
    pub identity: paintcraft_platform::source::SourceIdentity,
}
impl Loaded {
    pub fn same_resource(&self, other: &Self) -> bool {
        self.sha == other.sha && self.identity.matches(&other.identity)
    }
    pub fn protects(
        &self,
        path: &Path,
        candidate: Option<&paintcraft_platform::source::SourceIdentity>,
    ) -> bool {
        self.path == path || candidate.is_some_and(|candidate| self.identity.matches(candidate))
    }
    pub fn adopt(
        &self,
        preset: usize,
        policy: &str,
        quantize: bool,
        budget: &SharedBudget,
    ) -> Result<AdoptedPreset> {
        if self.source.brushes().len() > adoption::PRESETS_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "brush library exceeds preset allowance",
            ));
        }
        if policy != "native-grid-v1" {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "policy must be native-grid-v1",
            ));
        }
        let conversions = if quantize {
            &[RequiredConversion::QuantizedSpacing][..]
        } else {
            &[]
        };
        adoption::adopt_native(
            self.source.clone(),
            preset,
            StaticTipPolicy::NativeGridV1,
            conversions,
            budget,
            &|| Ok(()),
        )
    }
}
pub(super) fn acquire(
    path: &Path,
    expected: Option<&str>,
    frontend: registry::Frontend,
) -> Result<Loaded> {
    let mut request = Request::new(
        frontend,
        Operation::ImportAsset,
        Family::Abr,
        Platform::Native,
    );
    request.extension = path.extension().and_then(|s| s.to_str());
    registry::admission(request).require_allowed(request)?;
    if !path
        .metadata()
        .map_err(|e| storage("cannot inspect brush", &e))?
        .is_file()
    {
        return Err(Error::new(
            ErrorCode::StorageFailed,
            "brush input must be a regular file",
        ));
    }
    let file = File::open(path).map_err(|e| storage("cannot read brush", &e))?;
    let meta = file
        .metadata()
        .map_err(|e| storage("cannot inspect brush", &e))?;
    if !meta.is_file() || meta.len() > abr::BYTES_MAX as u64 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "brush source file/byte allowance",
        ));
    }
    let identity = paintcraft_platform::source::SourceIdentity::from_file(
        file.try_clone()
            .map_err(|e| storage("cannot retain brush source", &e))?,
    )?;
    let mut bytes = Vec::new();
    file.take(abr::BYTES_MAX as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| storage("cannot read brush", &e))?;
    if bytes.len() > abr::BYTES_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "brush exceeds byte allowance",
        ));
    }
    let sha = hex(&Sha256::digest(&bytes));
    guard(expected, &sha)?;
    Ok(Loaded {
        source: Arc::new(abr::decode(&bytes, &|| Ok(()))?),
        sha,
        path: path.into(),
        identity,
    })
}
pub(super) fn inspection(loaded: &Loaded) -> Result<serde_json::Value> {
    schema::inspection(loaded)
}
pub(super) fn schema() -> Result<Value> {
    core_value(&schema::schema()?)
}
fn core_value(value: &serde_json::Value) -> Result<Value> {
    paintcraft_core::types::json::parse(
        &serde_json::to_vec(value)
            .map_err(|_| Error::new(ErrorCode::InputSchema, "invalid brush JSON"))?,
    )
    .map_err(|_| Error::new(ErrorCode::InputSchema, "invalid brush JSON"))
}
pub fn run(options: Options, json: bool) -> Reply {
    match options.command {
        Command::Inspect(input) => bare(
            json,
            (|| {
                let loaded = acquire(
                    &input.input,
                    input.expect_input_sha256.as_deref(),
                    input.profile.frontend.into(),
                )?;
                Ok((
                    core_value(&inspection(&loaded)?)?,
                    format!("ABR library: {} presets", loaded.source.brushes().len()),
                ))
            })(),
        ),
        Command::Paint(p) => finish(
            json,
            (|| {
                save::preflight(&p.output.output, p.output.overwrite)?;
                let loaded = acquire(
                    &p.brush,
                    p.expect_brush_sha256.as_deref(),
                    p.profile.frontend.into(),
                )?;
                let candidate = paintcraft_platform::source::SourceIdentity::existing_destination(
                    &p.output.output,
                )?;
                if loaded.protects(&p.output.output, candidate.as_ref()) {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "painting output must not replace brush source",
                    ));
                }
                let budget = SharedBudget::new(BUDGET_BYTES);
                let preset =
                    loaded.adopt(p.preset, &p.policy, p.allow_spacing_quantization, &budget)?;
                let (bytes, sha, project_identity) =
                    read_regular_or_bounded(&p.project, INPUT_MAX, "project")?;
                guard(p.expect_input_sha256.as_deref(), &sha)?;
                let mut session =
                    super::persistence::session(super::source_metadata::read(&bytes)?);
                let (points, _) = read_bounded(&p.points, 4 << 20, "brush points")?;
                let stroke = Stroke {
                    points: serde_json::from_slice(&points).map_err(|_| {
                        Error::new(
                            ErrorCode::InputSchema,
                            "points must be an array of integer [x,y] pairs",
                        )
                    })?,
                    color: p.color,
                    point_scale: p.point_scale,
                    layer_id: p.layer_id,
                };
                let receipt = paint(&mut session, &preset, stroke, &budget)?;
                let in_place = same_file(&p.project, &p.output.output);
                let ctx = Context {
                    operation: if in_place { "save" } else { "save-as" },
                    source_state: session.document().state_id,
                    source_sha256: Some(sha),
                    losses_acknowledged: if p.allow_spacing_quantization {
                        vec!["abr-spacing-quantized".into()]
                    } else {
                        vec![]
                    },
                    destination: &p.output.output,
                };
                let (file, ok, summary) = write_native_guarded(
                    &p.output.output,
                    p.output.overwrite,
                    session.document(),
                    &ctx,
                    || {
                        let candidate =
                            paintcraft_platform::source::SourceIdentity::existing_destination(
                                &p.output.output,
                            )?;
                        if loaded.protects(&p.output.output, candidate.as_ref()) {
                            return Err(Error::new(
                                ErrorCode::InputSchema,
                                "painting output must not replace brush source",
                            ));
                        }
                        if let Some(project_identity) = &project_identity {
                            if !in_place && project_identity.matches_path(&p.output.output)? {
                                return Err(Error::new(
                                    ErrorCode::InputSchema,
                                    "copy output cannot replace the captured source project",
                                ));
                            }
                            project_identity.verify_bytes(
                                &p.project,
                                bytes.len() as u64,
                                &Sha256::digest(&bytes).into(),
                                &|| Ok(()),
                            )?;
                        }
                        Ok(())
                    },
                );
                Ok(Written {
                    engine: Some(receipt.to_value()),
                    receipt: file,
                    ok,
                    summary,
                })
            })(),
        ),
    }
}
