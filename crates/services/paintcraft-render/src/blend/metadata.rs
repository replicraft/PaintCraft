//! Pure pre-root metadata forecasts. No execution authority or retained grant.
//! Debt callbacks are accepted ONLY with actual Base AND Noise TLS absent.
use crate::prepared::{DocumentProgramsQuote, blend_programs::Keys};
use crate::{Document, Rect};
use paintcraft_core::types::{Error, ErrorCode, Result};

#[derive(Clone, Copy, Debug)]
pub struct UnboundProgramForecast {
    pub numerical: DocumentProgramsQuote,
    pub prepare_metadata_work: u64,
    /// Stored Noise, even disabled/default, requires future paired admission.
    pub requires_noise: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct UnboundCallForecast {
    pub context_bytes: u64,
    pub setup_work: u64,
    pub opaque_samples: u64,
    pub general_samples: u64,
    pub opaque_work: u64,
    pub general_work: u64,
    pub prepared_validation_work: u64,
    pub requires_noise: bool,
}
fn absent() -> Result<()> {
    if crate::spatial::current().is_some() || crate::noise_work::current().is_some() {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Unbound metadata forecast refuses installed execution authority",
        ));
    }
    Ok(())
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(super::limit)
}
/// Callback must bound cumulative issued metadata debt before each helper.
/// No compile, table allocation, root, admission or Programs is constructed.
/// Quote values do not grant bytes/work or authorize later rendering.
pub fn quote_unbound_programs_checked(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
    spend_metadata: &dyn Fn(u64) -> Result<()>,
) -> Result<UnboundProgramForecast> {
    absent()?;
    let check = || {
        absent()?;
        check()?;
        absent()
    };
    let spend = |n| {
        absent()?;
        spend_metadata(n)?;
        absent()
    };
    let (keys, requires_noise) = Keys::capture_unbound(doc, &check, &spend)?;
    let quote_metadata = keys.quote_metadata()?;
    spend(quote_metadata)?;
    let numerical = keys.quote()?;
    let prepare_metadata_work = add(
        add(keys.capture_work, quote_metadata)?,
        keys.constructor_metadata()?,
    )?;
    check()?;
    Ok(UnboundProgramForecast {
        numerical,
        prepare_metadata_work,
        requires_noise,
    })
}
/// Forecasts ONE fresh prepared owner and fresh context for this exact region.
/// No cached/TLS context reuse or borrowed-owner credit is inferred.
pub fn quote_unbound_call_checked(
    doc: &Document,
    region: Rect,
    check: &dyn Fn() -> Result<()>,
    spend_metadata: &dyn Fn(u64) -> Result<()>,
) -> Result<UnboundCallForecast> {
    absent()?;
    let check = || {
        absent()?;
        check()?;
        absent()
    };
    let spend = |n| {
        absent()?;
        spend_metadata(n)?;
        absent()
    };
    let (keys, requires_noise) = Keys::capture_unbound(doc, &check, &spend)?;
    spend(keys.quote_metadata()?)?;
    // Match the existing unbound call quote's paid bounded helper pass.
    let _ = keys.quote()?;
    let setup_work = (doc.roots.len() as u64)
        .checked_mul(256)
        .ok_or_else(super::limit)?;
    spend(setup_work)?;
    let quote = super::quote_document(doc, region)?;
    check()?;
    Ok(UnboundCallForecast {
        context_bytes: quote.control_bytes,
        setup_work,
        opaque_samples: quote.opaque_samples,
        general_samples: quote.general_samples,
        opaque_work: quote
            .opaque_samples
            .checked_mul(super::OPAQUE_EXTENSION_WORK)
            .ok_or_else(super::limit)?,
        general_work: quote
            .general_samples
            .checked_mul(super::GENERAL_WORK)
            .ok_or_else(super::limit)?,
        prepared_validation_work: keys.unbound_validation_work()?,
        requires_noise,
    })
}
mod rooted;
pub use rooted::{UnboundRootedCallForecast, quote_unbound_rooted_call_checked};
mod frame_workspace;
pub use frame_workspace::unbound_rooted_frame_workspace_control_bytes;
pub use frame_workspace::{
    UnboundRootedFrameWorkspace, quote_unbound_rooted_frame_workspace_checked,
};
#[cfg(test)]
mod tests;
