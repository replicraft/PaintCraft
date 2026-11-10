//! Owner-local canonical composition then encoded-RGB alpha-weighted reduction.
use super::*;
use crate::source8::display_public::OriginalControl8;
use paintcraft_core::types::{Rect, Rgba};
use paintcraft_render::display::{DisplayAccumulator, DisplayRequest};
use std::mem::size_of;
#[path = "display_owner/geometry.rs"]
mod geometry;
#[path = "display_owner/storage.rs"]
mod storage;
const OUTER_CONTROL: u64 = 4096;
fn invalid() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Source8 display declaration limit",
    )
}
fn layout_supported() -> bool {
    size_of::<Source8DisplayPatch>() == 72
        && size_of::<OriginalControl8<'static>>() == 16
        && std::mem::align_of::<OriginalControl8<'static>>() == 8
        && size_of::<DisplayRequest>() <= 40
        && size_of::<Result<DisplayRequest>>() <= 112
        && size_of::<geometry::Demand>() <= 128
        && size_of::<Result<geometry::Demand>>() <= 128
        && size_of::<storage::Scratch>() <= 128
        && size_of::<Result<storage::Scratch>>() <= 128
        && size_of::<Option<storage::Scratch>>() <= 128
        && size_of::<std::result::Result<(), std::collections::TryReserveError>>() <= 112
        && size_of::<DisplayAccumulator>() == 40
        && size_of::<Result<()>>() <= 112
        && size_of::<Result<u64>>() <= 112
        && size_of::<Result<Rgba>>() <= 112
        && size_of::<Result<Vec<u8>>>() <= 112
        && size_of::<Error>() <= 96
}
fn bootstrap() -> Result<()> {
    owner_ledger::validate_frame_bootstrap()?;
    let named = if cfg!(target_pointer_width = "64") {
        1880
    } else {
        1564
    } + 2 * size_of::<Source8DisplayPatch>()
        + 4 * size_of::<OriginalControl8<'static>>()
        + 112;
    let outer_named = if cfg!(target_pointer_width = "64") {
        3072
    } else {
        2928
    } + 4 * size_of::<OriginalControl8<'static>>();
    if !layout_supported()
        || named > owner_ledger::ENTRY_BYTES as usize
        || outer_named > OUTER_CONTROL as usize
    {
        return Err(invalid());
    }
    Ok(())
}
fn validate_originals(
    frame: &Source8FrameRealms<'_>,
    ctx: &frame_inputs::FrameValidation8<'_>,
    auth: &entry::EntryAuth8<'_>,
) -> Result<()> {
    for (scope, cap) in [
        (frame.control, frame.caps.control),
        (ctx.workspace, frame.caps.workspace),
        (frame.output, frame.caps.output),
    ] {
        auth.spend(112)?;
        let snapshot = scope.capacity_snapshot();
        if !snapshot.reserved || snapshot.ceiling != cap {
            return Err(invalid());
        }
    }
    if frame.caps.control < frame_inputs::CONTROL_BYTES {
        return Err(invalid());
    }
    frame_inputs::validate_declared(ctx, &frame.caps)
}
impl ClosedSource8 {
    pub(in crate::source8) fn render_display_closed_checked(
        &self,
        auth: &entry::EntryAuth8<'_>,
        required: &caller::RequiredCaller8,
        frame: &Source8FrameRealms<'_>,
        original_c: OriginalControl8<'_>,
        read_bytes: u64,
        display: Source8DisplayPatch,
    ) -> Result<Source8RgbaImage> {
        let OriginalControl8 {
            scope: c,
            capacity: c_cap,
        } = original_c;
        auth.spend(4096)?;
        bootstrap()?;
        let (base, retained, ceiling) = match required {
            caller::RequiredCaller8::Base(base) => (
                base,
                base.prepared_owner_union_bytes(None)?,
                base.bytes_limit(),
            ),
            caller::RequiredCaller8::Noise { selected, ambient } => (
                selected.workspace(),
                selected
                    .workspace()
                    .prepared_owner_union_bytes(Some(ambient.workspace()))?,
                selected.limits().bytes,
            ),
        };
        let validation = frame_inputs::FrameValidation8 {
            workspace: base.scope(),
            read_capacity_sum: read_bytes,
            caller_bytes: ceiling,
            prepared_union: retained,
        };
        frame_inputs::validate_declared(&validation, &frame.caps)?;
        auth.spend(112)?;
        let snapshot = c.capacity_snapshot();
        if !snapshot.reserved || snapshot.ceiling != c_cap {
            return Err(invalid());
        }
        auth.spend(8448)?;
        let mut control = c.account();
        control.resize(OUTER_CONTROL)?;
        validate_originals(frame, &validation, auth)?;
        auth.spend(8192)?;
        let doc = &self.payload.0.as_deref().ok_or_else(incomplete)?._document;
        let demand = geometry::demand(doc, display)?;
        if demand.output_peak > frame.caps.output {
            return Err(invalid());
        }
        // Future work is charged once before the first allocator/zeroing pass.
        auth.spend(demand.work)?;
        let mut scratch = if display.level == 0 {
            None
        } else {
            Some(storage::Scratch::new(
                demand.cells as usize,
                demand.accumulator_bytes,
                c,
                auth,
            )?)
        };
        let pending =
            frame_output::PendingOutput8::admit(demand.size[0], demand.size[1], frame, auth)?;
        let length = demand.cells as usize * 4;
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(length).map_err(|_| invalid())?;
        if pixels.capacity() != length {
            return Err(invalid());
        }
        pixels.resize(length, 0);
        let mut y = demand.native.y0;
        while y < demand.native.y1 {
            let bottom = ((y / 64 + 1) * 64).min(demand.native.y1);
            let mut x = demand.native.x0;
            while x < demand.native.x1 {
                auth.step(0)?;
                let right = ((x / 64 + 1) * 64).min(demand.native.x1);
                let tile = Rect {
                    x0: x,
                    y0: y,
                    x1: right,
                    y1: bottom,
                };
                let image = self.render_closed_checked(auth, required, frame, read_bytes, tile)?;
                let input = image.rgba();
                let width = tile.width() as usize;
                if display.level == 0 {
                    let mut row = 0usize;
                    while row < tile.height() as usize {
                        auth.step(0)?;
                        let source = row * width * 4;
                        let target = ((y - demand.native.y0 + row as i64) as usize
                            * demand.size[0] as usize
                            + (x - demand.native.x0) as usize)
                            * 4;
                        pixels[target..target + width * 4]
                            .copy_from_slice(&input[source..source + width * 4]);
                        row += 1;
                    }
                } else {
                    let sums = &mut scratch.as_mut().ok_or_else(invalid)?.values;
                    let mut i = 0usize;
                    while i < input.len() / 4 {
                        if i.is_multiple_of(64) {
                            auth.step(0)?;
                        }
                        let sx =
                            (x + (i % width) as i64 - demand.native.x0) / i64::from(demand.step);
                        let sy =
                            (y + (i / width) as i64 - demand.native.y0) / i64::from(demand.step);
                        let index = sy as usize * demand.size[0] as usize + sx as usize;
                        let offset = i * 4;
                        let rgba = Rgba([
                            input[offset],
                            input[offset + 1],
                            input[offset + 2],
                            input[offset + 3],
                        ]);
                        sums[index].add(rgba)?;
                        i += 1;
                    }
                }
                drop(image);
                x = right;
            }
            y = bottom;
        }
        auth.spend(1024)?;
        if let Some(scratch) = &scratch {
            let mut i = 0usize;
            while i < demand.cells as usize {
                if i.is_multiple_of(64) {
                    auth.step(0)?;
                }
                let x = demand.native.x0
                    + (i % demand.size[0] as usize) as i64 * i64::from(demand.step);
                let y = demand.native.y0
                    + (i / demand.size[0] as usize) as i64 * i64::from(demand.step);
                let samples = (demand.native.x1 - x).min(i64::from(demand.step)) as u64
                    * (demand.native.y1 - y).min(i64::from(demand.step)) as u64;
                if scratch.values[i].samples() != samples {
                    return Err(invalid());
                }
                pixels[i * 4..i * 4 + 4].copy_from_slice(&scratch.values[i].resolve()?.0);
                i += 1;
            }
        }
        let image = pending.finish(pixels, auth)?;
        drop(scratch);
        drop(control);
        Ok(image)
    }
}
