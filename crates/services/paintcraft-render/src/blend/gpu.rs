//! GPU-only request admission; CPU marker grants nothing here.
use super::*;
thread_local! {static GPU:Cell<Option<Context>>=const{Cell::new(None)};}
struct RestoreGpu(Option<Context>);
impl Drop for RestoreGpu {
    fn drop(&mut self) {
        GPU.with(|c| c.set(self.0));
    }
}
const fn controls() -> u64 {
    (std::mem::size_of::<Context>()
        + std::mem::size_of::<Option<Context>>()
        + std::mem::size_of::<RestoreGpu>()) as u64
}
pub(crate) fn allowed(doc: &Document) -> bool {
    GPU.with(|s| {
        s.get().is_some_and(|v| {
            v.address == doc as *const Document as usize
                && v.document == *doc.document_id.as_bytes()
                && v.state == *doc.state_id.as_bytes()
        })
    })
}
pub(crate) fn with_programs<T>(
    doc: &Document,
    region: Rect,
    check: &dyn Fn() -> Result<()>,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    // A CPU style stage is not a GPU shader/preservation capability.
    doc.require_no_styles()?;
    if !doc.has_non_normal_blends() {
        return crate::prepared::with_normal_document_programs(doc, check, work);
    }
    let base = crate::spatial::current().ok_or_else(missing)?;
    if allowed(doc) {
        let c = GPU.with(Cell::get).ok_or_else(missing)?;
        crate::validate_region(doc, region)?;
        quote::preflight(doc, region, c, Target::Gpu, check)?;
        return crate::prepared::with_gpu_document_programs(doc, check, work);
    }
    let bytes = controls();
    if bytes > base.bytes_limit() {
        return Err(limit());
    }
    let mut charge = base.scope().account();
    charge.resize(bytes)?;
    let setup = (doc.roots.len() as u64)
        .checked_mul(256)
        .ok_or_else(limit)?;
    if crate::noise_work::current().is_some() {
        crate::noise_work::current_for(&base)?.reserve_components(
            crate::noise_work::WorkComponents {
                base: setup,
                noise: 0,
            },
        )?;
    } else {
        base.reserve_work(setup)?;
    }
    let c = context(doc)?;
    let working_bytes = base.bytes_limit() - bytes;
    let child = base.with_byte_limit(working_bytes);
    crate::spatial::with_workspace(child, || {
        let previous = GPU.with(|s| s.replace(Some(c)));
        let _restore = RestoreGpu(previous);
        crate::validate_region(doc, region)?;
        quote::preflight(doc, region, c, Target::Gpu, check)?;
        crate::prepared::with_gpu_document_programs(doc, check, work)
    })
}
/// Private packed uniform inputs; no raw tag/bool constructor is exposed.
pub(crate) struct Dispatch {
    pub mode: Mode,
    pub opaque: bool,
}
pub(crate) fn dispatch(
    doc: &Document,
    layer: &Layer,
    index: usize,
    area: Rect,
) -> Result<Dispatch> {
    if layer.blend_mode == Mode::Normal {
        return Ok(Dispatch {
            mode: Mode::Normal,
            opaque: false,
        });
    }
    let c = GPU
        .with(Cell::get)
        .filter(|v| {
            v.address == doc as *const Document as usize
                && v.document == *doc.document_id.as_bytes()
                && v.state == *doc.state_id.as_bytes()
        })
        .ok_or_else(missing)?;
    let kind = kind_at(layer.blend_mode, index, c);
    reserve(kind, u64::try_from(area.area()).map_err(|_| limit())?)?;
    Ok(Dispatch {
        mode: layer.blend_mode,
        opaque: matches!(kind, Kind::Opaque(_)),
    })
}
