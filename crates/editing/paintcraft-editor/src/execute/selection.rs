//! Compile/admit immutable selections once and reuse exact views during replay.
use super::*;
use paintcraft_raster::selection::{BinaryMask, MaskLimits};

impl Stage<'_> {
    pub(super) fn mask_selection(&mut self, index: usize, op: &Op) -> Result<()> {
        if self.planning.is_none() {
            self.selection = self
                .selections
                .values
                .iter()
                .find(|(operation, _)| *operation == index)
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::InvariantViolation,
                        "compiled selection was not retained",
                    )
                })?
                .1
                .clone();
            return Ok(());
        }
        let domain = self.doc.canvas_rect();
        let budget = self.adm.selection_budget();
        let limits = MaskLimits::default();
        let mask = match op {
            Op::SelectionSetPolygon { points } => {
                BinaryMask::polygon(domain, points, &budget, limits, self.check)?
            }
            Op::SelectionFeather { radius } => {
                self.selection = paintcraft_raster::selection::region::refine(
                    domain,
                    &self.selection,
                    *radius,
                    &budget,
                    self.check,
                )?;
                return self.selections.push((index, self.selection.clone()));
            }
            Op::SelectionRegion {
                layer,
                points,
                settings,
            } => {
                let id = self.resolve(layer)?;
                use paintcraft_core::types::{resources::AllocationClass, selection::Shape};
                let needs_sample = matches!(settings.shape, Shape::Wand | Shape::Brush);
                let mut sample_charge = budget.scope(AllocationClass::SelectionMasks).account();
                let rendered = if needs_sample
                    && (settings.shape == Shape::Wand || settings.edge_aware)
                {
                    sample_charge.resize(u64::try_from(domain.area() * 4).map_err(|_| {
                        Error::new(ErrorCode::ResourceLimit, "Selection sample is too large")
                    })?)?;
                    let mut sampled = self.doc.clone();
                    if !settings.sample_merged {
                        sampled.roots = vec![id];
                        let layer = sampled.layers.get_mut(&id).expect("resolved layer");
                        layer.visible = true;
                        layer.blend_mode = paintcraft_core::types::LayerBlendMode::Normal;
                    }
                    for id in sampled.roots.clone() {
                        let layer = sampled.layers.get_mut(&id).expect("root layer");
                        layer.raster = self
                            .planning
                            .as_mut()
                            .expect("selection compilation")
                            .appearance_source(id, &layer.raster, &mut self.adm, self.check)?;
                    }
                    Some(paintcraft_render::render_cancellable(&sampled, self.check)?)
                } else {
                    None
                };
                self.selection = paintcraft_raster::selection::region::create(
                    domain,
                    &self.selection,
                    points,
                    *settings,
                    &budget,
                    self.check,
                    |x, y| {
                        Ok(rendered.as_ref().map_or(Rgba::TRANSPARENT, |pixels| {
                            let at =
                                ((y - domain.y0) * domain.width() + x - domain.x0) as usize * 4;
                            Rgba(pixels[at..at + 4].try_into().expect("canvas sample"))
                        }))
                    },
                )?;
                return self.selections.push((index, self.selection.clone()));
            }
            Op::SelectionInvert => {
                let mask = match &self.selection {
                    // With no active selection, Invert selects the whole canvas.
                    Selection::Inactive | Selection::Empty => BinaryMask::uniform(domain, false)?,
                    Selection::Rect(rect) => BinaryMask::uniform(*rect, true)?,
                    Selection::Mask(mask) => mask.clone(),
                };
                mask.reframed(domain, &budget, limits, self.check)?
                    .inverted()
            }
            _ => unreachable!("selection compiler operation"),
        };
        self.selection = Selection::Mask(mask);
        self.selections.push((index, self.selection.clone()))
    }
    pub(super) fn resize_selection(&mut self, dx: i64, dy: i64) -> Result<()> {
        self.selection = match &self.selection {
            Selection::Rect(rect) => rect
                .translated(dx, dy)
                .and_then(|rect| rect.intersect(&self.doc.canvas_rect()))
                .map_or(Selection::Empty, Selection::Rect),
            Selection::Mask(mask) => mask
                .translated(dx, dy)?
                .clipped(self.doc.canvas_rect())?
                .map_or(Selection::Empty, Selection::Mask),
            other => other.clone(),
        };
        Ok(())
    }
}
