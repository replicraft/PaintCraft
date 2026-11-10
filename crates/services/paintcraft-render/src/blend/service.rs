//! Explicit borrowed CPU blend service. No Worker endpoint or frontend grant.
//! External spend callbacks must immediately debit the SAME actual caller.
//! They grant no authority and cannot represent unpaid pre-root control debt.
use crate::noise_work::NoiseAdmission;
use crate::prepared::{
    DocumentProgramsQuote,
    blend_programs::{self, Keys},
};
use crate::spatial::WorkspaceAdmission;
use crate::{Document, Rect};
use paintcraft_core::types::{Error, ErrorCode, Result};
mod caller;
mod gateway;
mod rooted;
mod seeded;
pub use rooted::{RootedCallQuote, render_rooted_call_checked, rooted_call_quote_checked};
pub use seeded::{render_seeded_rooted_call_checked, seeded_rooted_call_quote_checked};
#[derive(Clone, Copy, Debug)]
pub enum Caller<'a> {
    Base(&'a WorkspaceAdmission),
    Noise(&'a NoiseAdmission),
}
#[derive(Clone, Copy, Debug)]
enum AllocationOrigin {
    NewInThisPreparation,
    BorrowedExisting,
}
/// Actual immutable prepared ownership; no attach/into-workspace escape.
#[derive(Debug)]
pub struct Programs {
    base: WorkspaceAdmission,
    noise: Option<NoiseAdmission>,
    origin: AllocationOrigin,
}
impl Programs {
    pub fn workspace(&self) -> &WorkspaceAdmission {
        &self.base
    }
    pub fn retained_bytes(&self) -> u64 {
        self.base.prepared_retained_bytes()
    }
    /// Actual newly allocated immutable payload, never a forecast/byte grant.
    pub fn newly_retained_bytes(&self) -> u64 {
        match self.origin {
            AllocationOrigin::NewInThisPreparation => self.retained_bytes(),
            AllocationOrigin::BorrowedExisting => 0,
        }
    }
    pub fn same_job(&self, caller: Caller<'_>) -> bool {
        match (&self.noise, caller) {
            (None, Caller::Base(a)) => self.base.same_job(a),
            (Some(a), Caller::Noise(b)) => a.same_job(b),
            _ => false,
        }
    }
    fn caller(&self) -> Caller<'_> {
        self.noise
            .as_ref()
            .map_or(Caller::Base(&self.base), Caller::Noise)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct ProgramQuote {
    pub numerical: DocumentProgramsQuote,
    pub compile_work: u64,
    pub required_retained_bytes: u64,
    pub new_retained_bytes: u64,
    pub preparation_peak_bytes: u64,
    pub prepare_metadata_work: u64,
}
#[derive(Clone, Copy, Debug)]
pub struct CallQuote {
    pub setup_work: u64,
    pub context_bytes: u64,
    pub required_retained_bytes: u64,
    pub opaque_samples: u64,
    pub general_samples: u64,
    pub opaque_work: u64,
    pub general_work: u64,
    pub call_metadata_work: u64,
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Blend service exceeds caller allowance",
    )
}
fn missing() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Blend service requires its matching caller",
    )
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(limit)
}
fn capture(
    doc: &Document,
    resolved: &caller::Resolved,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<Keys> {
    Keys::capture(
        doc,
        &resolved.base,
        resolved.ambient.as_ref(),
        resolved.noise.is_some(),
        check,
        spend,
    )
}
/// Pays quote metadata now; predicts preparation/compile without spending it.
/// `spend` must immediately debit this SAME actual caller. Pending pre-root debt
/// is not representable by this existing-Caller service and grants no credit.
pub fn program_quote_checked(
    doc: &Document,
    caller: Caller<'_>,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<ProgramQuote> {
    let resolved = caller::Resolved::new(caller)?;
    let debit = |n| resolved.metadata_spend(n, spend);
    let keys = capture(doc, &resolved, check, &debit)?;
    debit(keys.quote_metadata()?)?;
    let numerical = keys.quote()?;
    let borrowed = resolved.base.has_prepared_programs();
    let retained = if borrowed {
        resolved.required_retained_bytes()?
    } else {
        numerical.retained_bytes
    };
    let prepare_metadata_work = if borrowed {
        keys.validation_work(&resolved.base)?
    } else {
        add(
            add(keys.capture_work, keys.quote_metadata()?)?,
            keys.constructor_metadata()?,
        )?
    };
    check()?;
    Ok(ProgramQuote {
        numerical,
        compile_work: if borrowed { 0 } else { numerical.compile_work },
        required_retained_bytes: retained,
        new_retained_bytes: if borrowed { 0 } else { retained },
        preparation_peak_bytes: if borrowed {
            0
        } else {
            numerical
                .preparation_peak_bytes
                .max(blend_programs::CAPTURE_BYTES)
        },
        prepare_metadata_work,
    })
}
/// Same actual owner/scope/pair, no fresh pool or counter. Retained inline handle
/// bookkeeping is charged by the containing State/Frame, not an extra allocation.
pub fn prepare_programs_checked(
    doc: &Document,
    caller: Caller<'_>,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<Programs> {
    let resolved = caller::Resolved::new(caller)?;
    let debit = |n| resolved.metadata_spend(n, spend);
    if resolved.base.has_prepared_programs() {
        blend_programs::validate(
            doc,
            &resolved.base,
            resolved.ambient.as_ref(),
            resolved.noise.is_some(),
            check,
            &debit,
        )?;
        return Ok(Programs {
            base: resolved.base,
            noise: resolved.noise,
            origin: AllocationOrigin::BorrowedExisting,
        });
    }
    let bytes = blend_programs::CAPTURE_BYTES;
    if bytes > resolved.base.bytes_limit() {
        return Err(limit());
    }
    let mut capture_charge = resolved.base.scope().account();
    capture_charge.resize(bytes)?;
    let (base, noise) = {
        let keys = capture(doc, &resolved, check, &debit)?;
        debit(keys.quote_metadata()?)?;
        let quote = keys.quote()?;
        if quote.preparation_peak_bytes.max(bytes) > resolved.base.bytes_limit() {
            return Err(limit());
        }
        let metadata = keys.constructor_metadata()?;
        // Predict the complete remaining metadata+compile sum before helpers.
        resolved.check_compile(add(metadata, quote.compile_work)?)?;
        debit(metadata)?;
        let base = keys.prepare(resolved.base.clone(), check)?;
        let noise = resolved
            .noise
            .as_ref()
            .map(|n| n.for_workspace(&base))
            .transpose()?;
        check()?;
        (base, noise)
    };
    // Captured arrays end before their declaration releases.
    drop(capture_charge);
    let origin = if base.has_prepared_programs() {
        AllocationOrigin::NewInThisPreparation
    } else {
        AllocationOrigin::BorrowedExisting
    };
    Ok(Programs {
        base,
        noise,
        origin,
    })
}
/// Pays capture/proof metadata. Future call validation/setup/kernel remain deferred.
pub fn call_quote_checked(
    doc: &Document,
    region: Rect,
    caller: Caller<'_>,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<CallQuote> {
    let resolved = caller::Resolved::new(caller)?;
    let debit = |n| resolved.metadata_spend(n, spend);
    let keys = capture(doc, &resolved, check, &debit)?;
    let retained = if resolved.base.has_prepared_programs() {
        resolved.required_retained_bytes()?
    } else {
        debit(keys.quote_metadata()?)?;
        keys.quote()?.retained_bytes
    };
    let proof = (doc.roots.len() as u64)
        .checked_mul(256)
        .ok_or_else(limit)?;
    debit(proof)?;
    let quote = super::quote_document(doc, region)?;
    let fresh = !super::allowed(doc);
    check()?;
    Ok(CallQuote {
        setup_work: if fresh { proof } else { 0 },
        context_bytes: if fresh { quote.control_bytes } else { 0 },
        required_retained_bytes: retained,
        opaque_samples: quote.opaque_samples,
        general_samples: quote.general_samples,
        opaque_work: quote
            .opaque_samples
            .checked_mul(super::OPAQUE_EXTENSION_WORK)
            .ok_or_else(limit)?,
        general_work: quote
            .general_samples
            .checked_mul(super::GENERAL_WORK)
            .ok_or_else(limit)?,
        call_metadata_work: keys.validation_work(&resolved.base)?,
    })
}
/// Freshly validates exact keys and actual current minima before private CPU entry.
pub fn render_call_checked(
    doc: &Document,
    region: Rect,
    programs: &Programs,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let resolved = caller::Resolved::new(programs.caller())?;
    blend_programs::validate(
        doc,
        &resolved.base,
        resolved.ambient.as_ref(),
        resolved.noise.is_some(),
        check,
        &|n| resolved.spend(n),
    )?;
    // Origin is an internal ownership fact, never a caller-supplied credit flag.
    match programs.origin {
        AllocationOrigin::NewInThisPreparation | AllocationOrigin::BorrowedExisting => {}
    }
    resolved.install(|| gateway::render(doc, region, &resolved, check))
}
#[cfg(test)]
mod tests;
