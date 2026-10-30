//! Staged execution of a batch against a private copy of the document.
//! Nothing here publishes; the session installs the result atomically.

#[path = "sequential/canvas.rs"]
mod canvas;
#[path = "sequential/fill.rs"]
mod fill;
#[path = "sequential/layers.rs"]
mod layers;
#[path = "sequential/mask.rs"]
mod mask;
#[path = "sequential/paint.rs"]
mod paint;
#[path = "sequential/path.rs"]
mod path;
use path::walk_clipped;

use std::collections::{BTreeMap, HashMap};

use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba};
use uuid::Uuid;

use paintcraft_commands::{Batch, LayerRef, Mode, Op, ShapeKind};
use paintcraft_core::types::ids::deterministic_id;
use paintcraft_document::{COORD_ABS_MAX, Document, LAYER_COUNT_MAX, Layer};
use paintcraft_raster::shape::{self, CheckedSpanSet, Figure, Span};
use paintcraft_raster::{Admission, PaintPlan, RasterLayer};

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
        Selection::Mask(_) => unreachable!("historical oracle rejects mask snapshots"),
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
    execute_with_admission(base, selection, batch, Admission::new(staged_pixels))
}

/// Execute under an explicit staged-pixel and decoded-residency allocation realm.
/// Failed private staging leaves the source document unchanged.
pub fn execute_with_admission(
    base: &Document,
    selection: Selection,
    batch: &Batch,
    admission: Admission,
) -> Result<Staged> {
    execute_with_admission_checked(base, selection, batch, admission, &|| Ok(()))
}

/// Execute privately with cancellation between operations, flood classification
/// and planned painting.
pub fn execute_with_admission_checked(
    base: &Document,
    selection: Selection,
    batch: &Batch,
    admission: Admission,
    check: &dyn Fn() -> Result<()>,
) -> Result<Staged> {
    execute_with_geometry_cache(base, selection, batch, admission, None, check)
}

/// Optional disposable smooth-path geometry reuse for growing previews. Cached
/// sample unions contain no document pixels and cannot change the command result.
pub fn execute_with_geometry_cache(
    base: &Document,
    selection: Selection,
    batch: &Batch,
    admission: Admission,
    geometry: Option<&mut paintcraft_raster::smooth::PathCache>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Staged> {
    check()?;
    if matches!(selection, Selection::Mask(_)) {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "historical benchmark oracle does not implement mask selections",
        ));
    }
    let mut s = Stage {
        doc: base.clone(),
        selection,
        aliases: HashMap::new(),
        adm: admission,
        pencil_pixels: 0,
        pencil_segments: 0,
        stroke_rows: 0,
        base_state: base.state_id.to_string(),
        digest: &batch.digest,
        check,
        geometry,
    };
    for (index, op) in batch.operations.iter().enumerate() {
        check().and_then(|()| s.run(index, op)).map_err(|e| {
            if e.operation_index.is_some() {
                e
            } else {
                e.at(index)
            }
        })?;
    }
    check()?;
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
    check: &'a dyn Fn() -> Result<()>,
    geometry: Option<&'a mut paintcraft_raster::smooth::PathCache>,
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
            self.selection.clone(),
            self.doc.canvas_rect().intersect(&layer.extent()),
        )
    }

    fn charge_stroke(&mut self, points: &[(i64, i64)], size: u32, scale: u32) -> Result<()> {
        let length = path::length_scaled(points, scale, self.check)?;
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
        self.charge_stroke(points, size, 1)?;
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
        let mut set = CheckedSpanSet::with_scratch_limit(self.adm.stroke_scratch_limit());
        for (k, (a, b)) in path::segments(points).enumerate() {
            walk_clipped(a, b, Some(reach), k > 0, self.check, |x, y| {
                set.add_stamp(x, y, n, round, &clip, self.check)
            })?;
        }
        let mut plan = self.adm.paint_plan();
        set.visit_spans(self.check, |span| {
            self.add_paint_spans(id, &[span], color, mode, &mut plan)
        })?;
        self.apply_paint(id, &plan)
    }

    fn run(&mut self, index: usize, op: &Op) -> Result<()> {
        match op {
            Op::Filter(_) => {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Historical sequential baseline predates filter operations",
                ));
            }
            Op::CutSelection { .. }
            | Op::SelectionSetPolygon { .. }
            | Op::SelectionInvert
            | Op::CanvasCrop { .. }
            | Op::CanvasTransform { .. }
            | Op::TransformSelection { .. }
            | Op::MoveSelection { .. } => {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "historical benchmark oracle does not implement destructive crop",
                ));
            }
            Op::Contour { .. } => {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "historical benchmark oracle does not implement arbitrary contours",
                ));
            }
            Op::CanvasClear { alias, color } => self.clear_canvas(index, alias, *color)?,
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
                    filters: Default::default(),
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
                let mut plan = self.adm.paint_plan();
                plan.rect(local, *color, *mode)?;
                self.apply_paint(id, &plan)?;
            }
            Op::PencilPath {
                layer,
                points,
                color,
                mode,
            } => {
                let id = self.resolve(layer)?;
                let length = path::length(points, self.check)?;
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
                let mut plan = self.adm.paint_plan();
                for (n, (a, b)) in path::segments(points).enumerate() {
                    // A shared polyline joint is emitted once: later segments skip their first pixel.
                    walk_clipped(a, b, clip, n > 0, self.check, |x, y| {
                        plan.rect(
                            Rect {
                                x0: x - ox,
                                x1: x - ox + 1,
                                y0: y - oy,
                                y1: y - oy + 1,
                            },
                            *color,
                            *mode,
                        )
                    })?;
                }
                self.apply_paint(id, &plan)?;
            }
            Op::StrokePath {
                layer,
                points,
                color,
                mode,
                size,
                round,
                smooth,
                media,
                point_scale,
            } => {
                if *media != paintcraft_core::types::brush::BrushMedia::Plain {
                    return Err(Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "frozen baseline does not implement textured brushes",
                    ));
                }
                if !matches!(point_scale, 1 | 256) || (*point_scale != 1 && !smooth) {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "invalid stroke point scale",
                    ));
                }
                let id = self.resolve(layer)?;
                if *smooth {
                    self.charge_stroke(points, *size, *point_scale)?;
                    let Some(area) = self.editable(&self.doc.layers[&id]) else {
                        return Ok(());
                    };
                    // Rasterization unions overlapping geometry before coverage is applied.
                    let mut plan = self.adm.paint_plan();
                    let (ox, oy) = self.doc.layers[&id].origin;
                    let coverage = if let Some(cache) = self.geometry.as_deref_mut() {
                        cache.round_path_scaled(points, *size, &area, *point_scale, self.check)?
                    } else {
                        paintcraft_raster::smooth::round_path_scaled_checked(
                            points,
                            *size,
                            &area,
                            *point_scale,
                            self.check,
                        )?
                    };
                    for run in coverage {
                        (self.check)()?;
                        if *mode == Mode::Replace && run.samples < 16 {
                            plan.coverage_rect(
                                Rect {
                                    x0: run.x0 - ox,
                                    x1: run.x1 + 1 - ox,
                                    y0: run.y - oy,
                                    y1: run.y + 1 - oy,
                                },
                                *color,
                                run.samples,
                            )?;
                        } else {
                            let mut covered = *color;
                            covered.0[3] =
                                ((u16::from(color.a()) * u16::from(run.samples) + 8) / 16) as u8;
                            self.add_paint_spans(
                                id,
                                &[Span {
                                    y: run.y,
                                    x0: run.x0,
                                    x1: run.x1,
                                }],
                                covered,
                                *mode,
                                &mut plan,
                            )?;
                        }
                    }
                    self.apply_paint(id, &plan)?;
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
            } => self.flood_fill(layer, *at, *color, *mode, *tolerance)?,
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
                if matches!(kind, ShapeKind::Preset(_)) {
                    return Err(Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "historical benchmark oracle supports only line, rectangle and ellipse",
                    ));
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
                let width = x1.checked_sub(x0).and_then(|n| n.checked_add(1));
                let height = y1.checked_sub(y0).and_then(|n| n.checked_add(1));
                let bbox = width
                    .zip(height)
                    .and_then(|(w, h)| Rect::new(x0, y0, w, h))
                    .ok_or_else(|| Error::new(ErrorCode::InputSchema, "shape box overflows"))?;
                let fig = if *kind == ShapeKind::Rect {
                    Figure::Rect
                } else {
                    Figure::Ellipse
                };
                let mut plan = self.adm.paint_plan();
                if let Some(c) = fill {
                    shape::visit_filled(fig, &bbox, &clip, self.check, |span| {
                        self.add_paint_spans(id, &[span], *c, *mode, &mut plan)
                    })?;
                }
                if let Some((c, size)) = stroke {
                    shape::visit_outline(
                        fig,
                        &bbox,
                        i64::from(*size),
                        &clip,
                        self.check,
                        |span| self.add_paint_spans(id, &[span], *c, *mode, &mut plan),
                    )?;
                }
                self.apply_paint(id, &plan)?;
            }
            Op::MaskSpans {
                layer,
                color,
                mode,
                spans,
            } => self.paint_mask(layer, *color, *mode, spans)?,
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
                let mut plan = self.adm.paint_plan();
                paintcraft_raster::font::visit_layout(
                    text,
                    *at,
                    i64::from(*scale),
                    &clip,
                    self.check,
                    |span| self.add_paint_spans(id, &[span], *color, *mode, &mut plan),
                )?;
                self.apply_paint(id, &plan)?;
            }
            Op::SelectionSetRect { rect } => {
                self.selection = match self.doc.canvas_rect().intersect(rect) {
                    Some(r) => Selection::Rect(r),
                    None => Selection::Empty,
                };
            }
            Op::SelectionClear => self.selection = Selection::Inactive,
            Op::LayerExpand {
                layer,
                width,
                height,
            } => {
                let id = self.resolve(layer)?;
                self.doc
                    .layers
                    .get_mut(&id)
                    .expect("resolved")
                    .raster
                    .expand_to(*width, *height)?;
            }
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
