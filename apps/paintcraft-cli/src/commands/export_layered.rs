//! Fresh layered copies. Registry qualification remains authoritative.
use super::*;
use paintcraft_formats::{
    psd,
    registry::{self, Family, Frontend, Operation, Platform, Request},
};

#[derive(Clone, Copy, Debug, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub(super) enum Format {
    Psd,
    Psb,
}
impl Format {
    fn family(self) -> Family {
        match self {
            Self::Psd => Family::Psd,
            Self::Psb => Family::Psb,
        }
    }
    fn version(self) -> psd::Version {
        match self {
            Self::Psd => psd::Version::Psd,
            Self::Psb => psd::Version::Psb,
        }
    }
}
#[derive(Debug, clap::Args)]
pub struct Options {
    input: PathBuf,
    #[arg(long, value_enum)]
    format: Format,
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
            "layered copy extension must match explicit PSD/PSB format",
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
    format: Format,
    dest: &Path,
    overwrite: bool,
    allowed: &[String],
    source_sha: Option<String>,
    guard: &dyn Fn() -> Result<()>,
) -> Result<(Value, bool, String)> {
    let limits = psd::WriteLimits::default();
    let plan = psd::preflight(doc, format.version(), &limits, guard)?;
    plan.require_consent(allowed)?;
    let context = Context {
        operation: "export",
        source_state: doc.state_id,
        source_sha256: source_sha,
        losses_acknowledged: plan.losses,
        destination: dest,
    };
    let outcome = save::save_with(dest, overwrite, limits.encoded_bytes as u64, |mut sink| {
        guard()?;
        psd::write_document_to(doc, format.version(), &limits, allowed, &mut sink, guard)?;
        guard()
    });
    Ok(finish_output(dest, &context, outcome))
}
pub fn run(a: Options, json: bool) -> Reply {
    finish(
        json,
        (|| {
            let dest = &a.output.output;
            admit(a.profile.frontend.into(), a.format, dest)?;
            let allowed = a.losses.allowed_for(&[psd::TITLE_LOSS])?;
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
            let doc = pcnd::read(&bytes)?;
            let (receipt, ok, summary) = write_copy(
                &doc,
                a.format,
                dest,
                a.output.overwrite,
                &allowed,
                Some(sha),
                &protect,
            )?;
            Ok(Written {
                engine: None,
                receipt,
                ok,
                summary,
            })
        })(),
    )
}
#[cfg(test)]
mod tests;
