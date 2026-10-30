//! Explicit fresh MSP copies, retaining both admitted payload charges to publish.
use super::msp_options::Version;
use super::*;
use paintcraft_formats::{
    msp,
    registry::{self, Family, Frontend, Operation, Platform, Request},
};
use paintcraft_runtime::SharedBudget;
use std::cell::RefCell;
pub(super) fn admit(frontend: Frontend, path: &Path, version: Version) -> Result<()> {
    let mut request = Request::new(
        frontend,
        Operation::ExportFlat,
        Family::Msp,
        Platform::Native,
    );
    request.extension = path.extension().and_then(|value| value.to_str());
    if !request
        .extension
        .is_some_and(|value| value.eq_ignore_ascii_case("msp"))
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "MSP output requires .msp extension",
        ));
    }
    if version == Version::V2 {
        request.variant = Some(msp::NATIVE_V2_EXPORT_VARIANT);
    }
    match registry::admission(request) {
        registry::Decision::Allowed
        | registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent) => {
            Ok(())
        }
        decision => decision.require_allowed(request),
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn write_copy(
    doc: &Document,
    version: Version,
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
        version,
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
    version: Version,
    dest: &Path,
    overwrite: bool,
    allowed: &[String],
    source_sha: Option<String>,
    pool: SharedBudget,
    protect: &dyn Fn(&Path) -> Result<()>,
    check: &dyn Fn() -> Result<()>,
    policy: export::metadata_policy::Policy,
) -> Result<(Value, bool, String, Value)> {
    write_copy_with_policies(
        doc,
        version,
        dest,
        overwrite,
        allowed,
        source_sha,
        pool,
        protect,
        check,
        export::CopyPolicies {
            source_metadata: policy,
            ..Default::default()
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_copy_with_policies(
    doc: &Document,
    version: Version,
    dest: &Path,
    overwrite: bool,
    allowed: &[String],
    source_sha: Option<String>,
    pool: SharedBudget,
    protect: &dyn Fn(&Path) -> Result<()>,
    check: &dyn Fn() -> Result<()>,
    policies: export::CopyPolicies,
) -> Result<(Value, bool, String, Value)> {
    check()?;
    protect(dest)?;
    save::preflight(dest, overwrite)?;
    export::check_loss_codes(allowed)?;
    let render_lease = RefCell::new(None);
    let encode_lease = RefCell::new(None);
    let prepared = RefCell::new(None::<export::msp::PreparedMsp>);
    let outcome = save::save_with_preparation(
        dest,
        overwrite,
        msp::BYTES_MAX as u64,
        |sink| {
            check()?;
            protect(dest)?;
            let value = export::msp::prepare_document_with_policies(
                doc,
                version.backend(),
                allowed,
                |layout, source| {
                    let bytes = export::msp::render_reservation_bytes(layout)?
                        .checked_add(source)
                        .ok_or_else(|| {
                            Error::new(
                                ErrorCode::ResourceLimit,
                                "MSP source/render reservation overflow",
                            )
                        })?;
                    *render_lease.borrow_mut() = Some(pool.reserve(bytes)?);
                    Ok(())
                },
                |plan| {
                    *encode_lease.borrow_mut() =
                        Some(pool.reserve(plan.additional_bytes() as u64)?);
                    Ok(())
                },
                |doc, check| paintcraft_render::render_cancellable(doc, check),
                check,
                policies,
            )?;
            *prepared.borrow_mut() = Some(value);
            let value = prepared.borrow();
            for bytes in value.as_ref().unwrap().bytes().chunks(65536) {
                check()?;
                sink.write_all(bytes)
                    .map_err(|error| storage("staged MSP copy", &error))?;
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
    // Publication acknowledgement has no fallible source/context/cancel tail.
    let prepared = prepared.borrow();
    let actual = prepared
        .as_ref()
        .map_or_else(Vec::new, |value| value.losses().to_vec());
    let metadata = prepared.as_ref().map_or(Value::Null, |value| {
        let plan = value.plan();
        let layout = plan.layout();
        Value::object([
            ("format", Value::str("msp")),
            ("version", Value::str(version.id())),
            (
                "pixel_policy",
                Value::str("exact-opaque-black-white-msb-first"),
            ),
            (
                "row_policy",
                Value::str(if version == Version::V1 {
                    "uncompressed"
                } else {
                    "rle-nonzero-u16-length-map"
                }),
            ),
            ("width", Value::Int(i64::from(layout.width()))),
            ("height", Value::Int(i64::from(layout.height()))),
            ("packed_bytes", Value::Int(layout.packed_bytes() as i64)),
            ("encoded_bytes", Value::Int(plan.encoded_bytes() as i64)),
            (
                "encoded_sha256",
                Value::str(hex(&Sha256::digest(value.bytes()))),
            ),
            ("preserving_save", Value::Bool(false)),
        ])
    });
    let context = Context {
        operation: "export",
        source_state: doc.state_id,
        source_sha256: source_sha,
        losses_acknowledged: actual,
        destination: dest,
    };
    let (receipt, ok, summary) = finish_output(dest, &context, outcome);
    Ok((receipt, ok, summary, metadata))
}
#[cfg(test)]
mod tests;
