//! Staged execution of a batch against a private copy of the document.
//! Nothing here publishes; the session installs the result atomically.

mod canvas;
mod contour;
mod crop;
mod fill;
mod filter_masks;
mod filters;
mod staged;
pub use filters::PublishedAliases as FilterAliases;
mod layers;
mod mask;
mod paint;
mod path;
mod planning;
mod selection;
mod selection_move;
mod selection_transform;
mod transform;
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
/// Shared mask-filter traversal allowance across all paint plans in one batch.
pub const SELECTION_FILTER_WORK_MAX: u64 = 67_108_864;
/// Staged pixel storage per batch (the untrusted-decode soft limit).
pub use paintcraft_core::types::policy::STAGED_PIXELS_MAX;

pub use paintcraft_raster::Selection;

fn clip(selection: &Selection, area: Option<Rect>) -> Option<Rect> {
    match selection {
        Selection::Inactive => area,
        Selection::Empty => None,
        Selection::Rect(r) => area.and_then(|a| a.intersect(r)),
        Selection::Mask(mask) if mask.uniform_coverage() == Some(0) => None,
        Selection::Mask(mask) => area.and_then(|a| a.intersect(&mask.domain())),
    }
}
#[derive(Debug)]
pub struct Staged {
    pub doc: Document,
    pub selection: Selection,
    /// Layers created by the batch that still exist.
    pub aliases: BTreeMap<String, Uuid>,
    pub filter_aliases: FilterAliases,
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
    let mut s = Stage {
        doc: base.clone(),
        selection: selection.clone(),
        selections: planning::storage::Store::new(&admission),
        selection_geometries: planning::storage::Store::new(&admission),
        filter_masks: planning::storage::Store::new(&admission),
        coverage_work: SELECTION_FILTER_WORK_MAX,
        mask_replay: 0,
        aliases: HashMap::new(),
        filter_aliases: HashMap::new(),
        planning: Some(planning::Compiler::new(&admission, batch, check)?),
        operation: 0,
        adm: admission,
        pencil_pixels: 0,
        pencil_segments: 0,
        stroke_rows: 0,
        selection_work: SELECTION_FILTER_WORK_MAX,
        transform_reads: 0,
        base_state: base.state_id.to_string(),
        digest: &batch.digest,
        check,
        geometry,
    };
    for (index, op) in batch.operations.iter().enumerate() {
        s.operation = index;
        check().and_then(|()| s.run(index, op)).map_err(|e| {
            if e.operation_index.is_some() {
                e
            } else {
                e.at(index)
            }
        })?;
    }
    check()?;
    let (paints, footprint) = s.planning.take().expect("compiler stage").finish();
    // Logical source captures and simulated versions have dropped. Only original
    // compiled descriptors survive; reserve the complete new decoded peak once.
    s.adm
        .reserve_tiles(footprint.peak)
        .map_err(|e| e.at(footprint.peak_operation))?;
    s.doc = base.clone();
    s.selection = selection;
    s.aliases.clear();
    s.filter_aliases.clear();
    s.geometry = None;
    let (_charge, values) = paints.into_parts();
    let mut paints = values.into_iter().peekable();
    for (index, op) in batch.operations.iter().enumerate() {
        s.operation = index;
        check().map_err(|e| {
            if e.operation_index.is_some() {
                e
            } else {
                e.at(index)
            }
        })?;
        let result = (|| {
            match op {
                Op::CanvasCrop { rect } => s.crop_layout(*rect)?,
                Op::CanvasTransform { transform } => s.transform_layout(*transform)?,
                Op::TransformSelection { layer, .. } => s.selection_geometry_layout(layer)?,
                Op::MoveSelection { layer, offset, .. } => {
                    s.selection_move_layout(layer, *offset)?
                }
                _ => {}
            }
            if planning::is_paint(op) {
                while paints.peek().is_some_and(|paint| paint.operation == index) {
                    let paint = paints.next().expect("peeked paint");
                    match paint.plan {
                        planning::CompiledPlan::Paint(plan) => s.apply_paint(paint.layer, &plan)?,
                        planning::CompiledPlan::Mapped(plan) => {
                            s.doc
                                .layers
                                .get_mut(&paint.layer)
                                .expect("compiled mapped layer")
                                .raster = plan.replay(&mut s.adm, s.check)?;
                        }
                    }
                }
                Ok(())
            } else {
                s.run(index, op)
            }
        })();
        result.map_err(|e| {
            if e.operation_index.is_some() {
                e
            } else {
                e.at(index)
            }
        })?;
    }
    if s.mask_replay != s.filter_masks.values.len() {
        return Err(Error::new(
            ErrorCode::InvariantViolation,
            "Compiled filter masks were not replayed",
        ));
    }
    if paints.next().is_some() {
        return Err(Error::new(
            ErrorCode::InvariantViolation,
            "compiled paint was not replayed",
        ));
    }
    check()?;
    Ok(staged::finish(s))
}

struct Stage<'a> {
    doc: Document,
    selection: Selection,
    selections: planning::storage::Store<(usize, Selection)>,
    selection_geometries: planning::storage::Store<(usize, Option<selection_transform::Geometry>)>,
    filter_masks: planning::storage::Store<filter_masks::Captured>,
    coverage_work: u64,
    mask_replay: usize,
    /// Every alias ever declared in this batch; `None` once its layer is deleted.
    aliases: HashMap<String, Option<Uuid>>,
    filter_aliases: filters::Aliases,
    adm: Admission,
    pencil_pixels: i128,
    pencil_segments: i128,
    stroke_rows: i128,
    selection_work: u64,
    transform_reads: u64,
    base_state: String,
    digest: &'a str,
    check: &'a dyn Fn() -> Result<()>,
    geometry: Option<&'a mut paintcraft_raster::smooth::PathCache>,
    planning: Option<planning::Compiler>,
    operation: usize,
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
            &self.selection,
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
        let Some(set) = self.stroke_spans(id, points, size, round)? else {
            return Ok(());
        };
        let mut plan = self.adm.paint_plan();
        set.visit_spans(self.check, |span| {
            self.add_paint_spans(id, &[span], color, mode, &mut plan)
        })?;
        self.apply_paint(id, &plan)
    }

    fn stroke_spans(
        &mut self,
        id: Uuid,
        points: &[(i64, i64)],
        size: u32,
        round: bool,
    ) -> Result<Option<CheckedSpanSet>> {
        self.charge_stroke(points, size, 1)?;
        let Some(clip) = self.editable(&self.doc.layers[&id]) else {
            return Ok(None);
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
        Ok(Some(set))
    }

    fn run(&mut self, index: usize, op: &Op) -> Result<()> {
        match op {
            Op::Filter(op) => self.edit_filter(index, op)?,
            Op::SelectionSetPolygon { .. } | Op::SelectionInvert => {
                self.mask_selection(index, op)?
            }
            Op::CanvasCrop { rect } => self.crop_canvas(*rect)?,
            Op::CanvasTransform { transform } => self.transform_canvas(*transform)?,
            Op::MoveSelection {
                layer,
                offset,
                clear,
                transparent_key,
            } => {
                self.move_selected(layer, *offset, *clear, *transparent_key)?;
            }
            Op::TransformSelection {
                layer,
                transform,
                clear,
            } => self.transform_selected(layer, *transform, *clear)?,
            Op::Contour {
                layer,
                points,
                closed,
                stroke,
                fill,
                mode,
            } => {
                self.contour(layer, points, *closed, (*stroke, *fill, *mode))?;
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
                if let Some(planning) = &mut self.planning {
                    planning.remove(id);
                }
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
            Op::CutSelection { layer, clear } => {
                // Reuse the admitted paint compiler and exact mask clipping.
                self.run(
                    index,
                    &Op::FillRect {
                        layer: layer.clone(),
                        rect: self.doc.canvas_rect(),
                        color: *clear,
                        mode: Mode::Replace,
                    },
                )?;
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
                if !matches!(point_scale, 1 | 256)
                    || (*point_scale != 1
                        && !smooth
                        && *media == paintcraft_core::types::brush::BrushMedia::Plain)
                {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        "invalid stroke point scale",
                    ));
                }
                let id = self.resolve(layer)?;
                if *media != paintcraft_core::types::brush::BrushMedia::Plain {
                    if *mode != Mode::SourceOver {
                        return Err(Error::new(
                            ErrorCode::InputSchema,
                            "textured brushes require source-over",
                        ));
                    }
                    self.charge_stroke(points, *size, *point_scale)?;
                    let Some(area) = self.editable(&self.doc.layers[&id]) else {
                        return Ok(());
                    };
                    let spans = paintcraft_raster::media::coverage(
                        points,
                        *size,
                        *point_scale,
                        *media,
                        area,
                        self.check,
                    )?;
                    return if spans.is_empty() {
                        Ok(())
                    } else {
                        self.paint_mask(
                            layer,
                            *color,
                            paintcraft_commands::MaskMode::SourceOver,
                            &spans,
                        )
                    };
                }
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
                if let ShapeKind::Preset(preset) = kind {
                    let contour = shape::presets::contour(*preset, &bbox)?;
                    let mut plan = self.adm.paint_plan();
                    if let Some(c) = fill {
                        let mut emit =
                            |span| self.add_paint_spans(id, &[span], *c, *mode, &mut plan);
                        if width == Some(1) || height == Some(1) {
                            shape::visit_filled(Figure::Rect, &bbox, &clip, self.check, &mut emit)?;
                        } else {
                            shape::visit_polygon(&contour, &clip, self.check, &mut emit)?;
                        }
                    }
                    if let Some((c, size)) = stroke
                        && let Some(set) = self.stroke_spans(id, &contour, *size, true)?
                    {
                        set.visit_spans(self.check, |span| {
                            self.add_paint_spans(id, &[span], *c, *mode, &mut plan)
                        })?;
                    }
                    return self.apply_paint(id, &plan);
                }
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
                if let Some(planning) = &mut self.planning {
                    planning.expand(id, *width, *height, &self.adm, self.check)?;
                }
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
                self.resize_selection(dx, dy)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
