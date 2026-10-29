//! One admitted document composite retained through guarded DDS publication.
use super::*;
use paintcraft_formats::{
    dds,
    registry::{self, Family, Frontend, Operation, Platform, Request},
};
use paintcraft_runtime::SharedBudget;
use std::cell::RefCell;

pub(super) fn admit(frontend: Frontend, path: &Path) -> Result<()> {
    let mut request = Request::new(
        frontend,
        Operation::ExportFlat,
        Family::Dds,
        Platform::Native,
    );
    request.extension = path.extension().and_then(|value| value.to_str());
    request.variant = Some(dds::NATIVE_TEXTURE_EXPORT_VARIANT);
    match registry::admission(request) {
        registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent) => {
            Ok(())
        }
        decision => decision.require_allowed(request),
    }
}

#[allow(clippy::too_many_arguments)] // Explicit host guards and retained budget, no new executor.
pub(super) fn write_copy(
    doc: &Document,
    options: dds::TextureOptions,
    dest: &Path,
    overwrite: bool,
    allowed: &[String],
    source_sha: Option<String>,
    pool: SharedBudget,
    protect: &dyn Fn(&Path) -> Result<()>,
    check: &dyn Fn() -> Result<()>,
) -> Result<(Value, bool, String, Value)> {
    write_copy_with_metadata_policy(
        doc,
        options,
        dest,
        overwrite,
        allowed,
        source_sha,
        pool,
        protect,
        check,
        export::metadata_policy::Policy::default(),
    )
}
#[allow(clippy::too_many_arguments)]
pub(super) fn write_copy_with_metadata_policy(
    doc: &Document,
    options: dds::TextureOptions,
    dest: &Path,
    overwrite: bool,
    allowed: &[String],
    source_sha: Option<String>,
    pool: SharedBudget,
    protect: &dyn Fn(&Path) -> Result<()>,
    check: &dyn Fn() -> Result<()>,
    policy: export::metadata_policy::Policy,
) -> Result<(Value, bool, String, Value)> {
    check()?;
    protect(dest)?;
    save::preflight(dest, overwrite)?;
    export::check_loss_codes(allowed)?;
    let lease = RefCell::new(None);
    let prepared = RefCell::new(None::<export::dds::PreparedDds>);
    // Capture the expected destination before rendering or encoding. The same
    // immutable preparation and its admission lease survive staging/publication.
    let outcome = save::save_with_preparation(
        dest,
        overwrite,
        dds::BYTES_MAX as u64,
        |sink| {
            check()?;
            protect(dest)?;
            let value = export::dds::prepare_document_with_metadata_policy(
                doc,
                options,
                allowed,
                |plan, source| {
                    let bytes = export::dds::preparation_reservation_bytes(plan)?
                        .checked_add(source)
                        .ok_or_else(|| {
                            Error::new(
                                ErrorCode::ResourceLimit,
                                "DDS preparation reservation overflow",
                            )
                        })?;
                    *lease.borrow_mut() = Some(pool.reserve(bytes)?);
                    Ok(())
                },
                |doc, check| paintcraft_render::render_cancellable(doc, check),
                check,
                policy,
            )?;
            *prepared.borrow_mut() = Some(value);
            let value = prepared.borrow();
            for chunk in value.as_ref().unwrap().encoded().bytes().chunks(65536) {
                check()?;
                sink.write_all(chunk)
                    .map_err(|error| storage("staged DDS texture", &error))?;
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
            check()
        },
    );
    // No fallible source, context or cancellation recheck after publication.
    let prepared = prepared.borrow();
    let actual = prepared
        .as_ref()
        .map_or_else(Vec::new, |value| value.losses().to_vec());
    let details = prepared.as_ref().map_or(Value::Null, metadata);
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
fn metadata(value: &export::dds::PreparedDds) -> Value {
    let plan = value.encoded().plan();
    let options = plan.options();
    Value::object([
        ("format", Value::str("dds")),
        ("variant", Value::str(dds::NATIVE_TEXTURE_EXPORT_VARIANT)),
        (
            "encoding",
            Value::str(match options.encoding {
                dds::Encoding::Rgba8 => "rgba8",
                dds::Encoding::Bc1 => "bc1",
                dds::Encoding::Bc2 => "bc2",
                dds::Encoding::Bc3 => "bc3",
                _ => "unsupported",
            }),
        ),
        (
            "transfer",
            Value::str(if options.transfer == dds::Transfer::Srgb {
                "srgb"
            } else {
                "linear8"
            }),
        ),
        ("transfer_policy", Value::str(value.transfer().policy())),
        (
            "quantized_rgb_samples",
            Value::Int(value.transfer().quantized_rgb_samples() as i64),
        ),
        (
            "mip_request",
            Value::str(match options.mips {
                dds::MipCount::Full => "full",
                dds::MipCount::Levels(1) => "single",
                dds::MipCount::Levels(_) => "explicit-level-count",
            }),
        ),
        (
            "alpha",
            Value::str(if options.alpha == dds::SourceAlpha::Opaque {
                "opaque"
            } else {
                "straight"
            }),
        ),
        ("levels", Value::Int(plan.levels().len() as i64)),
        (
            "generated_levels",
            Value::Int(value.generated_levels() as i64),
        ),
        (
            "filter",
            Value::str(if options.filter == dds::MipFilter::Nearest {
                "nearest-base-centers"
            } else {
                "alpha-weighted-box-stored-base"
            }),
        ),
        (
            "effort",
            Value::str(if options.quality == dds::TextureQuality::Normal {
                "normal"
            } else {
                "high"
            }),
        ),
        (
            "compression_policy",
            value
                .encoded()
                .compression_policy()
                .map_or(Value::Null, Value::str),
        ),
        (
            "encoded_sha256",
            Value::str(hex(&Sha256::digest(value.encoded().bytes()))),
        ),
        ("preserving_save", Value::Bool(false)),
    ])
}
#[cfg(test)]
mod tests;
