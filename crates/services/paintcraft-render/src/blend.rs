//! Serial native CPU composition qualification. No GPU or wire authority.
use crate::*;
use paintcraft_core::types::LayerBlendMode as Mode;
use paintcraft_document::{Document, Layer};
use std::cell::Cell;
mod compose;
mod kernel;
mod quote;
pub(crate) use compose::{row, spatial};
pub use quote::{Quote, quote_document};
/// Additional logical delta over the existing Normal opaque skeleton.
pub const OPAQUE_EXTENSION_WORK: u64 = 32;
/// Conservative full general-call candidate; ledger review precedes adoption.
pub const GENERAL_WORK: u64 = 256;
#[derive(Clone, Copy)]
struct Context {
    address: usize,
    document: [u8; 16],
    state: [u8; 16],
    opaque_after: Option<usize>,
}
thread_local! {static CPU:Cell<Option<Context>>=const{Cell::new(None)};}
struct Restore(Option<Context>);
/// Actual target active-control declaration; inactive TLS/runtime bookkeeping is
/// excluded like existing context factories, not presented as allocator/RSS use.
pub const fn control_quote() -> u64 {
    (std::mem::size_of::<Context>()
        + std::mem::size_of::<Option<Context>>()
        + std::mem::size_of::<Restore>()) as u64
}

impl Drop for Restore {
    fn drop(&mut self) {
        CPU.with(|c| c.set(self.0));
    }
}
pub(crate) fn allowed(doc: &Document) -> bool {
    CPU.with(|c| {
        c.get().is_some_and(|v| {
            v.address == doc as *const Document as usize
                && v.document == *doc.document_id.as_bytes()
                && v.state == *doc.state_id.as_bytes()
        })
    })
}
/// One immutable full-canvas opacity certificate; no tile/pixel walk or owner.
fn context(doc: &Document) -> Result<Context> {
    if doc.roots.len() > paintcraft_document::LAYER_COUNT_MAX || doc.roots.len() != doc.layers.len()
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "CPU blend requires bounded flat roots",
        ));
    }
    let mut opaque_after = None;
    for (i, id) in doc.roots.iter().enumerate() {
        let l = doc
            .layers
            .get(id)
            .ok_or_else(|| Error::new(ErrorCode::InputSchema, "Blend root missing"))?;
        if l.id != *id {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "CPU blend root identity differs",
            ));
        }
        if opaque_after.is_none()
            && l.visible
            && l.opacity == 255
            && l.filters.is_empty()
            && !crate::layer_mask::active(l)
            && l.raster.resident_tiles() == 0
            && l.raster.default_pixel().a() == 255
            && l.extent().intersect(&doc.canvas_rect()) == Some(doc.canvas_rect())
        {
            opaque_after = Some(i);
        }
    }
    Ok(Context {
        address: doc as *const Document as usize,
        document: *doc.document_id.as_bytes(),
        state: *doc.state_id.as_bytes(),
        opaque_after,
    })
}
#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Normal,
    Opaque(Mode),
    General(Mode),
}
impl Kind {
    fn units(self) -> u64 {
        match self {
            Self::Normal => 0,
            Self::Opaque(_) => OPAQUE_EXTENSION_WORK,
            Self::General(_) => GENERAL_WORK,
        }
    }
}
fn kind_at(mode: Mode, index: usize, c: Context) -> Kind {
    if mode == Mode::Normal {
        Kind::Normal
    } else if c.opaque_after.is_some_and(|n| n < index) {
        Kind::Opaque(mode)
    } else {
        Kind::General(mode)
    }
}
pub(crate) fn kind(doc: &Document, layer: &Layer, index: usize) -> Result<Kind> {
    if layer.blend_mode == Mode::Normal {
        return Ok(Kind::Normal);
    }
    let c = CPU
        .with(Cell::get)
        .filter(|c| {
            c.address == doc as *const Document as usize
                && c.document == *doc.document_id.as_bytes()
                && c.state == *doc.state_id.as_bytes()
        })
        .ok_or_else(missing)?;
    Ok(kind_at(layer.blend_mode, index, c))
}
fn missing() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "CPU layer blends require an explicit caller workspace",
    )
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "CPU blend work exceeds caller admission",
    )
}
fn reserve(kind: Kind, samples: u64) -> Result<()> {
    if matches!(kind, Kind::Normal) {
        return Ok(());
    }
    let units = kind.units().checked_mul(samples).ok_or_else(limit)?;
    let base = crate::spatial::current().ok_or_else(missing)?;
    if crate::noise_work::current().is_some() {
        crate::noise_work::current_for(&base)?.reserve_components(
            crate::noise_work::WorkComponents {
                base: units,
                noise: 0,
            },
        )
    } else {
        base.reserve_work(units)
    }
}
pub(crate) fn with_programs<T>(
    doc: &Document,
    region: Rect,
    check: &dyn Fn() -> Result<()>,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    if !doc.has_non_normal_blends() {
        return crate::prepared::with_document_programs(doc, check, work);
    }
    let base = crate::spatial::current().ok_or_else(missing)?;
    if allowed(doc) {
        // Borrowed immutable opacity information may be reused. Work admission
        // never is: this can be a PUBLIC callback reentry with another ROI or
        // tightened child of the same cumulative parent.
        let c = CPU.with(Cell::get).ok_or_else(missing)?;
        crate::validate_region(doc, region)?;
        quote::preflight(doc, region, c, check)?;
        return crate::prepared::with_cpu_document_programs(doc, check, work);
    }
    let controls = control_quote();
    if controls > base.bytes_limit() {
        return Err(limit());
    }
    let mut control = base.scope().account();
    control.resize(controls)?;
    // Bounded once-per-CPU-request metadata control, never per sample/tile.
    let metadata = (doc.roots.len() as u64)
        .checked_mul(256)
        .ok_or_else(limit)?;
    if crate::noise_work::current().is_some() {
        crate::noise_work::current_for(&base)?.reserve_components(
            crate::noise_work::WorkComponents {
                base: metadata,
                noise: 0,
            },
        )?;
    } else {
        base.reserve_work(metadata)?;
    }
    let c = context(doc)?;
    // Same counter/factory, tightened bytes: active controls overlap all child
    // workspaces, including another document rendered by a caller callback.
    let working_bytes = base.bytes_limit() - controls;
    let child = base.with_byte_limit(working_bytes);
    crate::spatial::with_workspace(child, || {
        let prior = CPU.with(|s| s.replace(Some(c)));
        let _restore = Restore(prior);
        crate::validate_region(doc, region)?;
        quote::preflight(doc, region, c, check)?;
        crate::prepared::with_cpu_document_programs(doc, check, work)
    })
}
pub(crate) fn dense(kind: Kind, dst: &mut [u8], src: &[u8], opacity: u8) -> Result<()> {
    if matches!(kind, Kind::Normal) {
        crate::kernels::dense(dst, src, opacity);
        return Ok(());
    }
    if dst.len() != src.len() || !dst.len().is_multiple_of(4) {
        return Err(Error::new(
            ErrorCode::InvariantViolation,
            "CPU blend row extent",
        ));
    }
    reserve(kind, (src.len() / 4) as u64)?;
    for (d, s) in dst
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(src.as_chunks::<4>().0)
    {
        *d = kernel::pixel(kind, Rgba(*s).with_opacity(opacity), Rgba(*d))?.0;
    }
    Ok(())
}
pub(crate) fn pixel_precharged(kind: Kind, source: Rgba, destination: Rgba) -> Result<Rgba> {
    kernel::pixel(kind, source, destination)
}
pub(crate) fn reserve_pixels(kind: Kind, count: u64) -> Result<()> {
    reserve(kind, count)
}

#[cfg(test)]
mod request_tests;
