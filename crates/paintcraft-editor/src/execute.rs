//! Staged execution of a batch against a private copy of the document.
//! Nothing here publishes; the session installs the result atomically.

mod layers;

use std::collections::{BTreeMap, HashMap};

use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba};
use uuid::Uuid;

use paintcraft_commands::{Batch, LayerRef, Mode, Op, ShapeKind};
use paintcraft_core::types::ids::deterministic_id;
use paintcraft_document::{COORD_ABS_MAX, Document, LAYER_COUNT_MAX, Layer};
use paintcraft_raster::flood;
use paintcraft_raster::shape::{self, Figure, Span, SpanSet};
use paintcraft_raster::{Admission, RasterLayer};

/// Emitted-pixel and segment budgets for all pencil operations in one batch.
pub const PENCIL_PIXELS_MAX: i128 = 16_777_216;
pub const PENCIL_SEGMENTS_MAX: i128 = 1_048_576;
/// Stamp rows (path pixels × brush diameter) per batch for all brush strokes.
pub const STROKE_ROWS_MAX: i128 = 16_777_216;
/// Staged pixel storage per batch (the untrusted-decode soft limit).
pub use paintcraft_core::types::policy::STAGED_PIXELS_MAX;

pub use paintcraft_raster::Selection;

fn clip(selection: Selection, area: Option<Rect>) -> Option<Rect> {
    match selection {
        Selection::Inactive => area,
        Selection::Empty => None,
        Selection::Rect(r) => area.and_then(|a| a.intersect(&r)),
    }
}
#[derive(Debug)]
pub struct Staged {
    pub doc: Document,
    pub selection: Selection,
    /// Layers created by the batch that still exist.
    pub aliases: BTreeMap<String, Uuid>,
}

pub fn execute(base: &Document, selection: Selection, batch: &Batch) -> Result<Staged> {
    execute_with_budget(base, selection, batch, STAGED_PIXELS_MAX)
}

/// As [`execute`], with an explicit staged-pixel budget (used to inject exhaustion in tests).
pub fn execute_with_budget(
    base: &Document,
    selection: Selection,
    batch: &Batch,
    staged_pixels: u64,
) -> Result<Staged> {
    let mut s = Stage {
        doc: base.clone(),
        selection,
        aliases: HashMap::new(),
        adm: Admission::new(staged_pixels),
        pencil_pixels: 0,
        pencil_segments: 0,
        stroke_rows: 0,
        base_state: base.state_id.to_string(),
        digest: &batch.digest,
    };
    for (index, op) in batch.operations.iter().enumerate() {
        s.run(index, op).map_err(|e| {
            if e.operation_index.is_some() {
                e
            } else {
                e.at(index)
            }
        })?;
    }
    let aliases = s
        .aliases
        .into_iter()
        .filter_map(|(a, id)| {
            id.filter(|id| s.doc.layers.contains_key(id))
                .map(|id| (a, id))
        })
        .collect();
    Ok(Staged {
        doc: s.doc,
        selection: s.selection,
        aliases,
    })
}

struct Stage<'a> {
    doc: Document,
    selection: Selection,
    /// Every alias ever declared in this batch; `None` once its layer is deleted.
    aliases: HashMap<String, Option<Uuid>>,
    adm: Admission,
    pencil_pixels: i128,
    pencil_segments: i128,
    stroke_rows: i128,
    base_state: String,
    digest: &'a str,
}

impl Stage<'_> {
    fn resolve(&self, r: &LayerRef) -> Result<Uuid> {
        match r {
            LayerRef::Alias(a) => match self.aliases.get(a) {
                Some(Some(id)) => Ok(*id),
                _ => Err(Error::new(
                    ErrorCode::ReferenceUnknownAlias,
                    format!("unknown alias {a:?}"),
                )),
            },
            LayerRef::Id(id) if self.doc.layers.contains_key(id) => Ok(*id),
            LayerRef::Id(_) => Err(Error::new(
                ErrorCode::ReferenceUnknownId,
                "unknown layer id",
            )),
        }
    }

    /// Document-space area an edit of `layer` may touch: canvas ∩ layer ∩ selection.
    fn editable(&self, layer: &Layer) -> Option<Rect> {
        clip(
            self.selection,
            self.doc.canvas_rect().intersect(&layer.extent()),
        )
    }

    /// Paint absolute-coordinate spans of `id`, one blend per covered pixel.
    fn paint_spans(&mut self, id: Uuid, spans: &[Span], color: Rgba, mode: Mode) -> Result<()> {
        if mode == Mode::SourceOver && color.a() == 0 {
            return Ok(());
        }
        let l = self.doc.layers.get_mut(&id).expect("resolved");
        let (ox, oy) = l.origin;
        for s in spans {
            let r = Rect {
                x0: s.x0 - ox,
                x1: s.x1 + 1 - ox,
                y0: s.y - oy,
                y1: s.y + 1 - oy,
            };
            l.raster.fill_rect(&r, color, mode, &mut self.adm)?;
        }
        Ok(())
    }

    fn charge_stroke(&mut self, points: &[(i64, i64)], size: u32) -> Result<()> {
        let length: i128 = points
            .windows(2)
            .map(|w| i128::from((w[1].0 - w[0].0).abs().max((w[1].1 - w[0].1).abs())))
            .sum();
        self.pencil_pixels += 1 + length;
        self.pencil_segments += (points.len() as i128 - 1).max(1);
        self.stroke_rows += (1 + length) * i128::from(size);
        if self.pencil_pixels > PENCIL_PIXELS_MAX
            || self.pencil_segments > PENCIL_SEGMENTS_MAX
            || self.stroke_rows > STROKE_ROWS_MAX
        {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "stroke work budget exceeded",
            ));
        }
        Ok(())
    }

    /// A brush stroke along `points`: charge the budgets, then stamp, merge and paint.
    fn stroke(
        &mut self,
        id: Uuid,
        points: &[(i64, i64)],
        size: u32,
        round: bool,
        color: Rgba,
        mode: Mode,
    ) -> Result<()> {
        self.charge_stroke(points, size)?;
        let Some(clip) = self.editable(&self.doc.layers[&id]) else {
            return Ok(());
        };
        let n = i64::from(size);
        let reach = Rect {
            x0: clip.x0 - n,
            y0: clip.y0 - n,
            x1: clip.x1 + n,
            y1: clip.y1 + n,
        };
        let mut set = SpanSet::default();
        let segments: Vec<((i64, i64), (i64, i64))> = if points.len() == 1 {
            vec![(points[0], points[0])]
        } else {
            points.windows(2).map(|w| (w[0], w[1])).collect()
        };
        for (k, (a, b)) in segments.into_iter().enumerate() {
            walk_clipped(a, b, Some(reach), k > 0, |x, y| {
                set.add_stamp(x, y, n, round, &clip);
                Ok(())
            })?;
        }
        self.paint_spans(id, &set.into_spans(), color, mode)
    }

    fn run(&mut self, index: usize, op: &Op) -> Result<()> {
        match op {
            Op::LayerAdd {
                alias,
                name,
                width,
                height,
                origin,
                default,
            } => {
                if self.aliases.contains_key(alias) {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        format!("duplicate alias {alias:?}"),
                    ));
                }
                if self.doc.layers.len() >= LAYER_COUNT_MAX {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "layer count budget exceeded",
                    ));
                }
                let id = deterministic_id(&[
                    "layer",
                    &self.base_state,
                    self.digest,
                    &index.to_string(),
                    "0",
                ]);
                if self.doc.layers.contains_key(&id) {
                    return Err(Error::new(
                        ErrorCode::InvariantViolation,
                        "deterministic identifier collision",
                    ));
                }
                let layer = Layer {
                    id,
                    name: name.clone(),
                    origin: *origin,
                    visible: true,
                    opacity: 255,
                    raster: RasterLayer::new(*width, *height, *default),
                };
                self.doc.layers.insert(id, layer);
                self.doc.roots.push(id);
                self.aliases.insert(alias.clone(), Some(id));
            }
            Op::LayerDuplicate { layer, alias, name } => {
                self.duplicate_layer(index, layer, alias, name)?;
            }
            Op::LayerDelete { layer } => {
                let id = self.resolve(layer)?;
                self.doc.layers.remove(&id);
                self.doc.roots.retain(|r| *r != id);
                for target in self.aliases.values_mut().filter(|t| **t == Some(id)) {
                    *target = None;
                }
            }
            Op::LayerReorder { layer, to_index } => {
                let id = self.resolve(layer)?;
                if *to_index >= self.doc.roots.len() {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "to_index is beyond the top of the stack",
                    ));
                }
                self.doc.roots.retain(|r| *r != id);
                self.doc.roots.insert(*to_index, id);
            }
            Op::LayerSetProperties {
                layer,
                name,
                visible,
                opacity,
            } => {
                let id = self.resolve(layer)?;
                let l = self.doc.layers.get_mut(&id).expect("resolved");
                if let Some(n) = name {
                    l.name.clone_from(n);
                }
                if let Some(v) = visible {
                    l.visible = *v;
                }
                if let Some(o) = opacity {
                    l.opacity = *o;
                }
            }
            Op::FillRect {
                layer,
                rect,
                color,
                mode,
            } => {
                let id = self.resolve(layer)?;
                let l = &self.doc.layers[&id];
                let Some(area) = self.editable(l).and_then(|a| a.intersect(rect)) else {
                    return Ok(());
                };
                if *mode == Mode::SourceOver && color.a() == 0 {
                    return Ok(());
                }
                let (ox, oy) = l.origin;
                let local = area
                    .translated(-ox, -oy)
                    .expect("area lies inside the layer");
                let l = self.doc.layers.get_mut(&id).expect("resolved");
                l.raster.fill_rect(&local, *color, *mode, &mut self.adm)?;
            }
            Op::PencilPath {
                layer,
                points,
                color,
                mode,
            } => {
                let id = self.resolve(layer)?;
                let length: i128 = points
                    .windows(2)
                    .map(|w| i128::from((w[1].0 - w[0].0).abs().max((w[1].1 - w[0].1).abs())))
                    .sum();
                self.pencil_pixels += 1 + length;
                self.pencil_segments += (points.len() as i128 - 1).max(1);
                if self.pencil_pixels > PENCIL_PIXELS_MAX
                    || self.pencil_segments > PENCIL_SEGMENTS_MAX
                {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "pencil path work budget exceeded",
                    ));
                }
                let clip = self.editable(&self.doc.layers[&id]);
                let (ox, oy) = self.doc.layers[&id].origin;
                let l = self.doc.layers.get_mut(&id).expect("resolved");
                let segments: Vec<((i64, i64), (i64, i64))> = if points.len() == 1 {
                    vec![(points[0], points[0])]
                } else {
                    points.windows(2).map(|w| (w[0], w[1])).collect()
                };
                for (n, (a, b)) in segments.into_iter().enumerate() {
                    // A shared polyline joint is emitted once: later segments skip their first pixel.
                    walk_clipped(a, b, clip, n > 0, |x, y| {
                        l.raster
                            .blend_pixel(x - ox, y - oy, *color, *mode, &mut self.adm)
                    })?;
                }
            }
            Op::StrokePath {
                layer,
                points,
                color,
                mode,
                size,
                round,
                smooth,
            } => {
                let id = self.resolve(layer)?;
                if *smooth {
                    self.charge_stroke(points, *size)?;
                    let Some(area) = self.editable(&self.doc.layers[&id]) else {
                        return Ok(());
                    };
                    // Coverage scales source alpha once; all overlapping geometry
                    // was unioned by the rasterizer before any pixel is blended.
                    for run in paintcraft_raster::smooth::round_path(points, *size, &area)? {
                        if *mode == Mode::Replace && run.samples < 16 {
                            let l = self.doc.layers.get_mut(&id).expect("resolved");
                            for x in run.x0..=run.x1 {
                                let (lx, ly) = (x - l.origin.0, run.y - l.origin.1);
                                let dst = l.raster.get(lx, ly);
                                let mut mixed = *color;
                                for (out, (&src, &dst)) in
                                    mixed.0.iter_mut().zip(color.0.iter().zip(dst.0.iter()))
                                {
                                    *out = ((u16::from(src) * u16::from(run.samples)
                                        + u16::from(dst) * u16::from(16 - run.samples)
                                        + 8)
                                        / 16) as u8;
                                }
                                l.raster.blend_pixel(
                                    lx,
                                    ly,
                                    mixed,
                                    Mode::Replace,
                                    &mut self.adm,
                                )?;
                            }
                            continue;
                        }
                        let mut covered = *color;
                        covered.0[3] =
                            ((u16::from(color.a()) * u16::from(run.samples) + 8) / 16) as u8;
                        self.paint_spans(
                            id,
                            &[Span {
                                y: run.y,
                                x0: run.x0,
                                x1: run.x1,
                            }],
                            covered,
                            *mode,
                        )?;
                    }
                } else {
                    self.stroke(id, points, *size, *round, *color, *mode)?;
                }
            }
            Op::FloodFill {
                layer,
                at,
                color,
                mode,
                tolerance,
            } => {
                let id = self.resolve(layer)?;
                let l = &self.doc.layers[&id];
                let Some(area) = self.editable(l) else {
                    return Ok(());
                };
                let (ox, oy) = l.origin;
                let local = area
                    .translated(-ox, -oy)
                    .expect("area lies inside the layer");
                let spans = flood::region(&l.raster, (at.0 - ox, at.1 - oy), &local, *tolerance)?;
                let absolute: Vec<Span> = spans
                    .iter()
                    .map(|s| Span {
                        y: s.y + oy,
                        x0: s.x0 + ox,
                        x1: s.x1 + ox,
                    })
                    .collect();
                self.paint_spans(id, &absolute, *color, *mode)?;
            }
            Op::Shape {
                layer,
                kind,
                from,
                to,
                stroke,
                fill,
                mode,
            } => {
                let id = self.resolve(layer)?;
                if *kind == ShapeKind::Line {
                    let (color, size) = stroke.expect("validated at parse time");
                    return self.stroke(id, &[*from, *to], size, true, color, *mode);
                }
                let Some(clip) = self.editable(&self.doc.layers[&id]) else {
                    return Ok(());
                };
                let (x0, y0, x1, y1) = (
                    from.0.min(to.0),
                    from.1.min(to.1),
                    from.0.max(to.0),
                    from.1.max(to.1),
                );
                let bbox = Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1)
                    .ok_or_else(|| Error::new(ErrorCode::InputSchema, "shape box overflows"))?;
                let fig = if *kind == ShapeKind::Rect {
                    Figure::Rect
                } else {
                    Figure::Ellipse
                };
                if let Some(c) = fill {
                    self.paint_spans(id, &shape::filled(fig, &bbox, &clip), *c, *mode)?;
                }
                if let Some((c, size)) = stroke {
                    self.paint_spans(
                        id,
                        &shape::outline(fig, &bbox, i64::from(*size), &clip),
                        *c,
                        *mode,
                    )?;
                }
            }
            Op::Text {
                layer,
                at,
                text,
                scale,
                color,
                mode,
            } => {
                let id = self.resolve(layer)?;
                let Some(clip) = self.editable(&self.doc.layers[&id]) else {
                    return Ok(());
                };
                let spans = paintcraft_raster::font::layout(text, *at, i64::from(*scale), &clip)
                    .map_err(|c| {
                        Error::new(
                            ErrorCode::CapabilityUnsupported,
                            format!(
                                "character U+{:04X} has no glyph in the built-in font",
                                c as u32
                            ),
                        )
                    })?;
                self.paint_spans(id, &spans, *color, *mode)?;
            }
            Op::SelectionSetRect { rect } => {
                self.selection = match self.doc.canvas_rect().intersect(rect) {
                    Some(r) => Selection::Rect(r),
                    None => Selection::Empty,
                };
            }
            Op::SelectionClear => self.selection = Selection::Inactive,
            Op::CanvasResize {
                width,
                height,
                anchor,
            } => {
                let (ax, ay) = anchor.halves();
                let dx = ((i64::from(*width) - i64::from(self.doc.canvas.0)) * ax).div_euclid(2);
                let dy = ((i64::from(*height) - i64::from(self.doc.canvas.1)) * ay).div_euclid(2);
                for l in self.doc.layers.values_mut() {
                    let (x, y) = (l.origin.0 + dx, l.origin.1 + dy);
                    if x.abs() > COORD_ABS_MAX || y.abs() > COORD_ABS_MAX {
                        return Err(Error::new(
                            ErrorCode::ResourceLimit,
                            "translated layer origin exceeds coordinate budget",
                        ));
                    }
                    l.origin = (x, y);
                }
                self.doc.canvas = (*width, *height);
                if let Selection::Rect(r) = self.selection {
                    self.selection = match r
                        .translated(dx, dy)
                        .and_then(|t| t.intersect(&self.doc.canvas_rect()))
                    {
                        Some(t) => Selection::Rect(t),
                        None => Selection::Empty,
                    };
                }
            }
        }
        Ok(())
    }
}

/// The symmetric all-octant Bresenham recurrence, yielding only pixels inside
/// `clip`. The walk is monotonic, so it stops once the remainder cannot re-enter
/// the clip box. Every visited pixel is exactly one the unclipped walk emits.
fn walk_clipped(
    a: (i64, i64),
    b: (i64, i64),
    clip: Option<Rect>,
    skip_first: bool,
    mut emit: impl FnMut(i64, i64) -> Result<()>,
) -> Result<()> {
    let Some(clip) = clip else { return Ok(()) };
    let (mut x, mut y) = a;
    let (x1, y1) = b;
    let dx = (x1 - x).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let dy = -(y1 - y).abs();
    let sy = if y < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut first = true;
    loop {
        if x.min(x1) >= clip.x1
            || x.max(x1) < clip.x0
            || y.min(y1) >= clip.y1
            || y.max(y1) < clip.y0
        {
            return Ok(());
        }
        if !(first && skip_first) && clip.contains(x, y) {
            emit(x, y)?;
        }
        first = false;
        if (x, y) == (x1, y1) {
            return Ok(());
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_commands::Mode;
    use paintcraft_core::types::Rgba;
    use paintcraft_document::{POLICY, new_document};

    fn batch_of(operations: Vec<Op>) -> Batch {
        Batch {
            version: 1,
            policy: POLICY.into(),
            base_state: uuid::Uuid::nil(),
            base_epoch: 0,
            seed: None,
            operations,
            digest: "0".repeat(64),
        }
    }

    fn add(alias: &str, w: u32, h: u32) -> Op {
        Op::LayerAdd {
            alias: alias.into(),
            name: alias.into(),
            width: w,
            height: h,
            origin: (0, 0),
            default: Rgba::TRANSPARENT,
        }
    }

    fn pencil(alias: &str, points: Vec<(i64, i64)>) -> Op {
        Op::PencilPath {
            layer: LayerRef::Alias(alias.into()),
            points,
            color: Rgba([1, 2, 3, 255]),
            mode: Mode::Replace,
        }
    }

    #[test]
    fn pencil_segment_budget_counts_zero_length_segments() {
        let doc = new_document(2, 2, None, 0, POLICY).unwrap();
        // 17 × 65,535 segments exceeds 1,048,576 while emitting a single pixel each time.
        let mut ops = vec![add("a", 2, 2)];
        ops.extend((0..17).map(|_| pencil("a", vec![(0, 0); 65_536])));
        let e = execute(&doc, Selection::Inactive, &batch_of(ops.clone())).unwrap_err();
        assert_eq!(e.code, ErrorCode::ResourceLimit);
        assert_eq!(
            e.operation_index,
            Some(17),
            "the first operation that crosses the limit"
        );
        // Sixteen of them stay within budget.
        ops.truncate(17);
        assert!(execute(&doc, Selection::Inactive, &batch_of(ops)).is_ok());
    }

    #[test]
    fn pencil_pixel_budget_is_checked_before_expansion() {
        let doc = new_document(2, 2, None, 0, POLICY).unwrap();
        let far = vec![(0, 0), (PENCIL_PIXELS_MAX as i64, 0)];
        let e = execute(
            &doc,
            Selection::Inactive,
            &batch_of(vec![add("a", 2, 2), pencil("a", far)]),
        )
        .unwrap_err();
        assert_eq!(
            (e.code, e.operation_index),
            (ErrorCode::ResourceLimit, Some(1))
        );
        // Exactly the limit is admitted: 1 + (limit - 1) pixels.
        let edge = vec![(0, 0), (PENCIL_PIXELS_MAX as i64 - 1, 0)];
        assert!(
            execute(
                &doc,
                Selection::Inactive,
                &batch_of(vec![add("a", 2, 2), pencil("a", edge)])
            )
            .is_ok()
        );
    }

    #[test]
    fn exhausted_staging_aborts_with_a_resource_error() {
        let doc = new_document(512, 512, None, 0, POLICY).unwrap();
        let fill = Op::FillRect {
            layer: LayerRef::Alias("a".into()),
            rect: Rect::new(0, 0, 512, 512).unwrap(),
            color: Rgba([9, 9, 9, 255]),
            mode: Mode::Replace,
        };
        let batch = batch_of(vec![add("a", 512, 512), fill]);
        let e = execute_with_budget(&doc, Selection::Inactive, &batch, 65_536 * 3).unwrap_err();
        assert_eq!(
            (e.code, e.operation_index),
            (ErrorCode::ResourceLimit, Some(1))
        );
        assert!(execute_with_budget(&doc, Selection::Inactive, &batch, 65_536 * 4).is_ok());
    }

    fn full_walk(a: (i64, i64), b: (i64, i64)) -> Vec<(i64, i64)> {
        let mut out = Vec::new();
        walk_clipped(a, b, Rect::new(-1000, -1000, 2000, 2000), false, |x, y| {
            out.push((x, y));
            Ok(())
        })
        .unwrap();
        out
    }

    #[test]
    fn bresenham_covers_endpoints_in_every_octant() {
        for end in [
            (6, 4),
            (5, 6),
            (2, 6),
            (0, 5),
            (0, 2),
            (1, 0),
            (4, 0),
            (6, 1),
            (3, 3),
        ] {
            let w = full_walk((3, 3), end);
            assert_eq!(w.first(), Some(&(3, 3)));
            assert_eq!(w.last(), Some(&end));
            assert_eq!(w.len() as i64, 1 + (end.0 - 3).abs().max((end.1 - 3).abs()));
        }
        assert_eq!(full_walk((2, 2), (2, 2)), [(2, 2)]);
    }

    #[test]
    fn clipped_walk_equals_the_filtered_full_walk() {
        // Deterministic LCG; avoids a dev-dependency for randomness.
        let mut s = 0x2545_F491_4F6C_DD1Du64;
        let mut next = |m: i64| {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((s >> 33) as i64 % (2 * m + 1)) - m
        };
        for _ in 0..5000 {
            let (a, b) = ((next(15), next(15)), (next(15), next(15)));
            let c0 = (next(8), next(8));
            let clip = Rect::new(c0.0, c0.1, 1 + next(5).abs() + 3, 1 + next(5).abs() + 3).unwrap();
            for skip in [false, true] {
                let want: Vec<_> = full_walk(a, b)
                    .into_iter()
                    .skip(usize::from(skip))
                    .filter(|&(x, y)| clip.contains(x, y))
                    .collect();
                let mut got = Vec::new();
                walk_clipped(a, b, Some(clip), skip, |x, y| {
                    got.push((x, y));
                    Ok(())
                })
                .unwrap();
                assert_eq!(got, want, "{a:?}->{b:?} clip {clip:?} skip {skip}");
            }
        }
    }
}
