//! Native fixed lossless RGB8 fresh copies; no generic ImageFormat encoder.
use super::*;
use paintcraft_formats::{
    codec_worker::{self as wire, jxr_encode as encode},
    registry::{self, Family, Frontend, Operation, Platform, Request},
};
use paintcraft_runtime::SharedBudget;
use std::{cell::RefCell, sync::Arc};
#[derive(Debug, clap::Args)]
pub struct Options {
    input: PathBuf,
    #[command(flatten)]
    metadata: super::export_metadata::Options,
    #[arg(long)]
    expect_input_sha256: Option<String>,
    #[command(flatten)]
    profile: format_profile::Options,
    #[command(flatten)]
    losses: LossOptions,
    #[command(flatten)]
    output: Output,
}
pub(super) fn admit(frontend: Frontend, path: &Path) -> Result<()> {
    let mut request = Request::new(
        frontend,
        Operation::ExportFlat,
        Family::Jxr,
        Platform::Native,
    );
    request.extension = path.extension().and_then(|value| value.to_str());
    if request.extension.is_none() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "JXR output requires .jxr, .wdp or .wmp extension",
        ));
    }
    request.variant = Some(encode::qualified::EXPORT_VARIANT);
    match registry::admission(request) {
        registry::Decision::Allowed
        | registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent) => {
            Ok(())
        }
        decision => decision.require_allowed(request),
    }
}
/// Encoding runs within the coordinator's captured destination window. Expensive
/// source guards run before/after staging; cheap cancellation runs during codec IO.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_copy(
    doc: &Document,
    dest: &Path,
    overwrite: bool,
    allowed: &[String],
    source_sha: Option<String>,
    owner: wire::Owner,
    current: &dyn Fn() -> Result<wire::Owner>,
    pool: SharedBudget,
    protect: &dyn Fn(&Path) -> Result<()>,
    check: jxr_export_job::Check,
) -> Result<(Value, bool, String, Value)> {
    write_copy_with_metadata_policy(
        doc,
        dest,
        overwrite,
        allowed,
        source_sha,
        owner,
        current,
        pool,
        protect,
        check,
        export::metadata_policy::Policy::default(),
    )
}
#[allow(clippy::too_many_arguments)]
pub(super) fn write_copy_with_metadata_policy(
    doc: &Document,
    dest: &Path,
    overwrite: bool,
    allowed: &[String],
    source_sha: Option<String>,
    owner: wire::Owner,
    current: &dyn Fn() -> Result<wire::Owner>,
    pool: SharedBudget,
    protect: &dyn Fn(&Path) -> Result<()>,
    check: jxr_export_job::Check,
    policy: export::metadata_policy::Policy,
) -> Result<(Value, bool, String, Value)> {
    check()?;
    policy.validate(
        doc,
        export::metadata_policy::Target::new(Family::Jxr, Operation::ExportFlat),
    )?;
    protect(dest)?;
    save::preflight(dest, overwrite)?;
    export::check_loss_codes(allowed)?;
    let prepared = RefCell::new(None::<jxr_export_job::Prepared>);
    let outcome = save::save_with_preparation(
        dest,
        overwrite,
        encode::ENCODED_BYTES_MAX,
        |sink| {
            check()?;
            protect(dest)?;
            if current()? != owner {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "JXR export owner changed",
                ));
            }
            let value = if policy == export::metadata_policy::Policy::Refuse {
                jxr_export_job::prepare(doc.clone(), owner, pool, check.clone(), allowed.to_vec())?
            } else {
                jxr_export_job::prepare_with_metadata_policy(
                    doc.clone(),
                    owner,
                    pool,
                    check.clone(),
                    allowed.to_vec(),
                    policy,
                )?
            };
            value.check(current)?;
            *prepared.borrow_mut() = Some(value);
            let guard = prepared.borrow();
            let value = guard.as_ref().unwrap();
            for bytes in value.encoded.bytes().chunks(65536) {
                check()?;
                sink.write_all(bytes)
                    .map_err(|error| storage("staged JXR copy", &error))?;
            }
            check()
        },
        |outcome| {
            check()?;
            protect(dest)?;
            if let Some(resolved) = outcome.resolved_destination()
                && resolved != dest
            {
                protect(resolved)?;
            }
            prepared
                .borrow()
                .as_ref()
                .ok_or_else(|| {
                    Error::new(ErrorCode::InvariantViolation, "JXR prepared result missing")
                })?
                .check(current)
        },
    );
    let guard = prepared.borrow();
    let actual = guard
        .as_ref()
        .map_or_else(Vec::new, |value| value.losses.clone());
    let details = Value::object([
        ("format", Value::str("jxr")),
        (
            "encoder_profile",
            Value::str(encode::qualified::NATIVE_PROFILE),
        ),
        ("pixel_profile", Value::str(encode::PROFILE)),
        ("density_policy", Value::str(encode::DENSITY_POLICY)),
        (
            "helper_sha256",
            guard.as_ref().map_or(Value::Null, |value| {
                Value::str(hex(&value.encoded.helper_sha256()))
            }),
        ),
        (
            "encoded_sha256",
            guard.as_ref().map_or(Value::Null, |value| {
                Value::str(hex(&value.encoded.sha256()))
            }),
        ),
        ("preserving_save", Value::Bool(false)),
    ]);
    let context = Context {
        operation: "export",
        source_state: doc.state_id,
        source_sha256: source_sha,
        losses_acknowledged: actual,
        destination: dest,
    };
    let (receipt, ok, summary) = finish_output(dest, &context, outcome);
    Ok((receipt, ok, summary, details))
}
pub fn run(a: Options, json: bool) -> Reply {
    finish(
        json,
        (|| {
            let dest = &a.output.output;
            admit(a.profile.frontend.into(), dest)?;
            let allowed = a.losses.allowed()?;
            let (bytes, sha, identity) = read_captured(&a.input, INPUT_MAX, "JXR source project")?;
            guard(a.expect_input_sha256.as_deref(), &sha)?;
            if a.profile.admit_open(&a.input, &bytes)? != Family::Pcnd {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "JXR export expects a PCND project",
                ));
            }
            let protect = |destination: &Path| {
                if identity.matches_path(destination)? {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "JXR copy must not replace its captured source project",
                    ));
                }
                Ok(())
            };
            let doc = super::source_metadata::read(&bytes)?;
            drop(bytes);
            let mut owner = codec_worker::owner();
            owner.state = doc.state_id;
            let policy = a
                .metadata
                .capture(&doc, super::export_metadata::CopyFormat::Jxr)?;
            let (receipt, ok, summary, details) =
                if a.metadata.intent == export::metadata_policy::Intent::Refuse {
                    write_copy(
                        &doc,
                        dest,
                        a.output.overwrite,
                        &allowed,
                        Some(sha),
                        owner,
                        &|| Ok(owner),
                        SharedBudget::new(512 << 20),
                        &protect,
                        Arc::new(|| Ok(())),
                    )?
                } else {
                    write_copy_with_metadata_policy(
                        &doc,
                        dest,
                        a.output.overwrite,
                        &allowed,
                        Some(sha),
                        owner,
                        &|| Ok(owner),
                        SharedBudget::new(512 << 20),
                        &protect,
                        Arc::new(|| Ok(())),
                        policy,
                    )?
                };

            Ok(Written {
                engine: Some(details),
                receipt,
                ok,
                summary,
            })
        })(),
    )
}

#[cfg(test)]
mod tests;
