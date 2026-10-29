//! Fresh layered copies. Registry qualification remains authoritative.
use super::*;
use paintcraft_formats::{
    paint, pdn, psd,
    registry::{self, Family, Frontend, Operation, Platform, Request},
};

#[derive(Clone, Copy, Debug, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub(super) enum Format {
    Psd,
    Psb,
    Pdn,
    Paint,
}
impl Format {
    fn family(self) -> Family {
        match self {
            Self::Psd => Family::Psd,
            Self::Psb => Family::Psb,
            Self::Pdn => Family::Pdn,
            Self::Paint => Family::Paint,
        }
    }
    pub(super) fn target(
        self,
        background: Option<Background>,
        geometry: Option<Geometry>,
        paint_fill: Option<[u16; 4]>,
        paint_names: Option<PaintNames>,
    ) -> Result<Target> {
        if !matches!(self, Self::Pdn) && (background.is_some() || geometry.is_some()) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "PDN policies require PDN output",
            ));
        }
        if !matches!(self, Self::Paint) && (paint_fill.is_some() || paint_names.is_some()) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "PAINT policies require PAINT output",
            ));
        }
        match self {
            Self::Pdn => Ok(Target::Pdn(pdn::Options {
                background: match background.ok_or_else(|| {
                    Error::new(
                        ErrorCode::InputSchema,
                        "PDN copy requires an explicit pdn-background choice",
                    )
                })? {
                    Background::Ordinary => pdn::BackgroundRole::Ordinary,
                    Background::Bottom => pdn::BackgroundRole::Bottom,
                },
                geometry: match geometry.unwrap_or(Geometry::Exact) {
                    Geometry::Exact => pdn::Geometry::Exact,
                    Geometry::Materialize => pdn::Geometry::Materialize,
                },
            })),
            Self::Paint => Ok(Target::Paint(paint::Options {
                canvas_fill: paint_fill.ok_or_else(|| {
                    Error::new(
                        ErrorCode::InputSchema,
                        "PAINT copy requires explicit paint-canvas-fill RGBA16",
                    )
                })?,
                layer_names: match paint_names.ok_or_else(|| {
                    Error::new(
                        ErrorCode::InputSchema,
                        "PAINT copy requires explicit paint-names policy",
                    )
                })? {
                    PaintNames::RequireEmpty => paint::LayerNames::RequireEmpty,
                    PaintNames::Omit => paint::LayerNames::Omit,
                },
            })),
            Self::Psd | Self::Psb => Ok(Target::Psd(if matches!(self, Self::Psd) {
                psd::Version::Psd
            } else {
                psd::Version::Psb
            })),
        }
    }
    fn losses(self) -> &'static [&'static str] {
        match self {
            Self::Psd | Self::Psb => &[psd::TITLE_LOSS],
            Self::Pdn => pdn::KNOWN_LOSSES,
            Self::Paint => paint::KNOWN_LOSSES,
        }
    }
}
#[derive(Clone, Copy, Debug, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Background {
    Ordinary,
    Bottom,
}
#[derive(Clone, Copy, Debug, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Geometry {
    Exact,
    Materialize,
}
#[derive(Clone, Copy, Debug, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PaintNames {
    RequireEmpty,
    Omit,
}
fn parse_paint_fill(text: &str) -> std::result::Result<[u16; 4], String> {
    let mut fields = text.split(',');
    let mut fill = [0u16; 4];
    for value in &mut fill {
        *value = fields
            .next()
            .ok_or("PAINT canvas fill requires four comma-separated RGBA16 values")?
            .trim()
            .parse::<u16>()
            .map_err(|_| "PAINT canvas fill components must be 0..65535")?;
    }
    if fields.next().is_some() {
        return Err("PAINT canvas fill requires exactly four RGBA16 values".into());
    }
    Ok(fill)
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Target {
    Psd(psd::Version),
    Pdn(pdn::Options),
    Paint(paint::Options),
}
type CopyOutcome = (Value, bool, String, Option<Value>);
#[derive(Debug, clap::Args)]
pub struct Options {
    input: PathBuf,
    #[arg(long, value_enum)]
    format: Format,
    /// PDN requires an explicit role; ordinary faithfully retains normal native layers.
    #[arg(long, value_enum)]
    pdn_background: Option<Background>,
    /// PDN materialization pads/places each layer; geometry/clipping losses need consent.
    #[arg(long, value_enum)]
    pdn_geometry: Option<Geometry>,
    /// PAINT requires four explicit 0..65535 RGBA fill fields, comma-separated.
    #[arg(long, value_parser=parse_paint_fill,value_name="R,G,B,A")]
    paint_canvas_fill: Option<[u16; 4]>,
    /// PAINT requires empty UI names or explicit omission, never guessed item/UI mapping.
    #[arg(long, value_enum)]
    paint_names: Option<PaintNames>,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[command(flatten)]
    profile: format_profile::Options,
    #[command(flatten)]
    losses: LossOptions,
    #[command(flatten)]
    output: Output,
}
/// Validate the explicit version and registry qualification before acquiring input.
/// No caller can resolve an unavailable writer by acknowledging conversion losses.
pub(super) fn admit(frontend: Frontend, format: Format, dest: &Path) -> Result<()> {
    let extension = dest.extension().and_then(|value| value.to_str());
    if extension.and_then(Family::from_extension) != Some(format.family()) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "layered copy extension must match explicit PSD/PSB/PDN/PAINT format",
        ));
    }
    let mut request = Request::new(
        frontend,
        Operation::SaveAsCopy,
        format.family(),
        Platform::Native,
    );
    request.extension = extension;
    registry::admission(request).require_allowed(request)
}
/// Private post-admission implementation, shared by CLI and MCP. Writes through
/// the existing bounded staged save; guard runs both before encoding and before
/// returning control to the atomic publication service.
pub(super) fn write_copy(
    doc: &Document,
    target: Target,
    dest: &Path,
    overwrite: bool,
    allowed: &[String],
    source_sha: Option<String>,
    guard: &dyn Fn() -> Result<()>,
) -> Result<CopyOutcome> {
    match target {
        Target::Psd(version) => {
            let limits = psd::WriteLimits::default();
            let plan = psd::preflight(doc, version, &limits, guard)?;
            plan.require_consent(allowed)?;
            let context = Context {
                operation: "export",
                source_state: doc.state_id,
                source_sha256: source_sha,
                losses_acknowledged: plan.losses,
                destination: dest,
            };
            let outcome = save::save_with_preparation(
                dest,
                overwrite,
                limits.encoded_bytes as u64,
                |mut sink| {
                    guard()?;
                    psd::write_document_to(doc, version, &limits, allowed, &mut sink, guard)?;
                    guard()
                },
                |_| guard(),
            );
            let (receipt, ok, summary) = finish_output(dest, &context, outcome);
            Ok((receipt, ok, summary, None))
        }
        Target::Pdn(options) => {
            let limits = pdn::Limits::default();
            let prepared = pdn::prepare_document(doc, options, &limits, allowed, guard)?;
            let context = Context {
                operation: "export",
                source_state: doc.state_id,
                source_sha256: source_sha,
                losses_acknowledged: prepared.plan().losses.clone(),
                destination: dest,
            };
            let outcome = save::save_with_preparation(
                dest,
                overwrite,
                limits.writer.encoded_bytes as u64,
                |mut sink| {
                    guard()?;
                    prepared.write_to(allowed, &mut sink, guard)?;
                    guard()
                },
                |_| guard(),
            );
            let (receipt, ok, summary) = finish_output(dest, &context, outcome);
            Ok((receipt, ok, summary, Some(prepared.plan().receipt())))
        }
        Target::Paint(options) => {
            let limits = paint::Limits::default();
            let prepared = paint::prepare_document(doc, options, &limits, allowed, guard)?;
            let context = Context {
                operation: "export",
                source_state: doc.state_id,
                source_sha256: source_sha,
                losses_acknowledged: prepared.plan().losses.clone(),
                destination: dest,
            };
            let outcome = save::save_with_preparation(
                dest,
                overwrite,
                limits.writer.encoded_bytes as u64,
                |mut sink| {
                    guard()?;
                    prepared.write_to(allowed, &mut sink, guard)?;
                    guard()
                },
                |_| guard(),
            );
            let (receipt, ok, summary) = finish_output(dest, &context, outcome);
            Ok((receipt, ok, summary, Some(prepared.plan().receipt())))
        }
    }
}

pub fn run(a: Options, json: bool) -> Reply {
    finish(
        json,
        (|| {
            let dest = &a.output.output;
            admit(a.profile.frontend.into(), a.format, dest)?;
            let target = a.format.target(
                a.pdn_background,
                a.pdn_geometry,
                a.paint_canvas_fill,
                a.paint_names,
            )?;
            let allowed = a.losses.allowed_for(a.format.losses())?;
            let (bytes, sha, identity) = read_captured(&a.input, INPUT_MAX, "layered copy source")?;
            guard(a.expect_input_sha256.as_deref(), &sha)?;
            let protect = || {
                if identity.matches_path(dest)? {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "layered copy must not replace its captured source project",
                    ));
                }
                Ok(())
            };
            protect()?;
            save::preflight(dest, a.output.overwrite)?;
            if a.profile.admit_open(&a.input, &bytes)? != Family::Pcnd {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "layered copy expects a PCND project",
                ));
            }
            let doc = super::source_metadata::read(&bytes)?;
            let (receipt, ok, summary, preparation) = write_copy(
                &doc,
                target,
                dest,
                a.output.overwrite,
                &allowed,
                Some(sha),
                &protect,
            )?;
            Ok(Written {
                engine: preparation,
                receipt,
                ok,
                summary,
            })
        })(),
    )
}
#[cfg(test)]
mod tests;
