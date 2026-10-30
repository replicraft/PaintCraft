//! Typed fresh ICNS assets; explicit logical size and density, never inferred.
use super::*;
use paintcraft_formats::{
    icns, icons,
    registry::{self, Family, Operation, Platform, Request},
};
#[derive(clap::Args, Debug)]
pub struct Options {
    input: PathBuf,
    #[command(flatten)]
    metadata: super::export_metadata::Options,
    /// Repeat an exact ICNS type: ic07/ic08/ic09/ic10/ic11/ic12/ic13/ic14.
    /// For example ic11 is 16 logical pixels at 2x, whereas ic12 is 32 at 2x.
    #[arg(long="type", required=true,value_parser=parse_kind)]
    kinds: Vec<icns::IconType>,
    /// Fit each entry independently on transparent padding from the original canvas.
    #[arg(long, value_enum, default_value = "lanczos3")]
    resampling: Resampling,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[command(flatten)]
    profile: format_profile::Options,
    #[command(flatten)]
    losses: LossOptions,
    #[command(flatten)]
    output: Output,
}
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum Resampling {
    Nearest,
    Lanczos3,
}
fn parse_kind(value: &str) -> std::result::Result<icns::IconType, String> {
    let tag: [u8; 4] = value
        .as_bytes()
        .try_into()
        .map_err(|_| "ICNS type must be an exact four-byte tag")?;
    icns::IconType::from_tag(tag).filter(|kind|kind.writable()).ok_or_else(||"qualified ICNS PNG types are ic07, ic08, ic09, ic10, ic11, ic12, ic13, ic14; no type or Retina substitution is implicit".into())
}
pub fn run(a: Options, json: bool) -> Reply {
    finish(
        json,
        (|| {
            let dest = &a.output.output;
            let mut request = Request::new(
                a.profile.frontend.into(),
                Operation::SaveAsCopy,
                Family::Icns,
                Platform::Native,
            );
            request.extension = dest.extension().and_then(|value| value.to_str());
            if request.extension.is_none() {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "ICNS output requires .icns extension",
                ));
            }
            request.variant = Some("icon-fresh-entry-set");
            match registry::admission(request) {
                registry::Decision::Allowed
                | registry::Decision::AllowedWithPreflight(
                    registry::Reason::ConvertRequiresConsent,
                ) => {}
                decision => decision.require_allowed(request)?,
            }
            // The only resolved preflight is the exact prepared pixel/loss consent below.
            let allowed = a.losses.allowed_for(icons::KNOWN_EXPORT_LOSSES)?;
            let (bytes, sha, identity) = read_captured(&a.input, INPUT_MAX, "ICNS source project")?;
            guard(a.expect_input_sha256.as_deref(), &sha)?;
            if identity.matches_path(dest)? {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "ICNS output must not replace its captured source project",
                ));
            }
            save::preflight(dest, a.output.overwrite)?;
            if a.profile.admit_open(&a.input, &bytes)? != Family::Pcnd {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "ICNS export expects a PCND project",
                ));
            }
            let doc = super::source_metadata::read(&bytes)?;
            let filter = match a.resampling {
                Resampling::Nearest => icons::Filter::Nearest,
                Resampling::Lanczos3 => icons::Filter::Lanczos3,
            };
            let policy = a.metadata.capture_policies(
                &doc,
                super::export_metadata::CopyFormat::Icns,
                a.losses.filter_bake_intent(),
                &|| Ok(()),
            )?;
            let prepared =
                icns::prepare_document_with_policies(&doc, &a.kinds, filter, &|| Ok(()), policy)?;
            let encoded = prepared.encode(&allowed, &|| Ok(()))?;
            // Check the same captured source immediately before output publication.
            if identity.matches_path(dest)? {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "ICNS output must not replace its captured source project",
                ));
            }
            let context = Context {
                operation: "export",
                source_state: doc.state_id,
                source_sha256: Some(sha),
                losses_acknowledged: prepared.losses().to_vec(),
                destination: dest,
            };
            let (receipt, ok, summary) =
                write_output(dest, a.output.overwrite, Ok(encoded), &context);
            Ok(Written {
                engine: None,
                receipt,
                ok,
                summary,
            })
        })(),
    )
}
