//! Checked scalar demand, never an allocation grant or a new source recipe.
use super::*;
use paintcraft_render::display::DisplayRequest;
#[repr(C)]
pub(super) struct Demand {
    pub native: Rect,
    pub size: [u32; 2],
    pub step: u32,
    pub cells: u64,
    pub accumulator_bytes: u64,
    pub work: u64,
    pub output_peak: u64,
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(invalid)
}
fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(invalid)
}
fn coordinate(base: i64, cell: i64, step: i64) -> Result<i64> {
    base.checked_add(cell.checked_mul(step).ok_or_else(invalid)?)
        .ok_or_else(invalid)
}
pub(super) fn demand(
    doc: &paintcraft_document::Document,
    d: Source8DisplayPatch,
) -> Result<Demand> {
    let view = DisplayRequest::new(doc, d.visible, d.level)?;
    let size = view.size();
    let p = d.patch;
    if p.x0 < 0
        || p.y0 < 0
        || p.x1 <= p.x0
        || p.y1 <= p.y0
        || p.x1 > i64::from(size[0])
        || p.y1 > i64::from(size[1])
        || p.width() > 64
        || p.height() > 64
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Source8 display patch cells",
        ));
    }
    let region = view.region();
    let step = i64::from(view.step());
    let native = Rect {
        x0: coordinate(region.x0, p.x0, step)?,
        y0: coordinate(region.y0, p.y0, step)?,
        x1: coordinate(region.x0, p.x1, step)?.min(region.x1),
        y1: coordinate(region.y0, p.y1, step)?.min(region.y1),
    };
    let cells = mul(p.width() as u64, p.height() as u64)?;
    let native_pixels = mul(native.width() as u64, native.height() as u64)?;
    let tx = add(native.x1 as u64, 63)? / 64 - native.x0 as u64 / 64;
    let ty = add(native.y1 as u64, 63)? / 64 - native.y0 as u64 / 64;
    let tiles = mul(tx, ty)?;
    let accumulator_bytes = if d.level == 0 { 0 } else { mul(cells, 40)? };
    // Row helper preflights every cell once, then writes without per-pixel Results.
    let per_native = if d.level == 0 { 8 } else { 256 };
    let row_work = if d.level == 0 {
        0
    } else {
        // DisplayRequest bounds region.area<=u64::MAX/65536. Each canonical
        // column/group contains at least one native pixel, so rows,groups<=N.
        // Hence1024*rows+128*groups<=1152*N<u64::MAX, including wasm32.
        let rows = native.height() as u64 * tx;
        let groups = native.height() as u64
            * if view.step() <= 64 {
                p.width() as u64
            } else {
                tx
            };
        rows * 1024 + groups * 128
    };
    let resolves = if d.level == 0 {
        mul(256, mul(native.height() as u64, tx)?)?
    } else {
        mul(cells, 512)?
    };
    let work = add(
        add(add(mul(native_pixels, per_native)?, resolves)?, row_work)?,
        add(add(accumulator_bytes, mul(cells, 4)?)?, mul(tiles, 4096)?)?,
    )?;
    let native_peak = mul(
        mul(
            native.width().min(64) as u64,
            native.height().min(64) as u64,
        )?,
        4,
    )?;
    let output_peak = add(add(mul(cells, 4)?, native_peak)?, 224)?;
    Ok(Demand {
        native,
        size: [p.width() as u32, p.height() as u32],
        step: step as u32,
        cells,
        accumulator_bytes,
        work,
        output_peak,
    })
}
