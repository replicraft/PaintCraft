//! Apply resolved font coverage through selection clipping and raster admission.
use super::*;
use paintcraft_core::types::text::MaskSpan;
impl Stage<'_> {
    pub(super) fn paint_mask(
        &mut self,
        layer: &LayerRef,
        color: Rgba,
        spans: &[MaskSpan],
    ) -> Result<()> {
        paintcraft_commands::validate_mask_spans(spans)?;
        self.pencil_pixels += spans
            .iter()
            .map(|s| i128::from(s.x1 - s.x0 + 1))
            .sum::<i128>();
        if self.pencil_pixels > PENCIL_PIXELS_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "mask pixel budget exceeded",
            ));
        }
        let id = self.resolve(layer)?;
        let mut plan = self.adm.paint_plan();
        if let Some(clip) = self.editable(&self.doc.layers[&id]) {
            for span in spans {
                if span.y < clip.y0 || span.y >= clip.y1 {
                    continue;
                }
                let x0 = span.x0.max(clip.x0);
                let x1 = span.x1.min(clip.x1 - 1);
                if x0 > x1 {
                    continue;
                }
                let mut pixel = color;
                pixel.0[3] = ((u16::from(color.a()) * u16::from(span.coverage) + 127) / 255) as u8;
                self.add_paint_spans(
                    id,
                    &[Span { y: span.y, x0, x1 }],
                    pixel,
                    Mode::SourceOver,
                    &mut plan,
                )?;
            }
        }
        self.apply_paint(id, &plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coverage_multiplies_alpha_once_and_respects_origin_selection_and_atomic_budget() {
        let mut base = paintcraft_document::new_document(
            8,
            8,
            Some(Rgba::TRANSPARENT),
            1,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let id = base.roots[0];
        base.layers.get_mut(&id).unwrap().origin = (2, 1);
        let batch = Batch {
            version: 2,
            policy: paintcraft_document::POLICY.into(),
            base_state: base.state_id,
            base_epoch: 0,
            seed: None,
            digest: "0".repeat(64),
            operations: vec![Op::MaskSpans {
                layer: LayerRef::Id(id),
                color: Rgba([200, 100, 50, 128]),
                spans: vec![MaskSpan {
                    y: 2,
                    x0: 0,
                    x1: 7,
                    coverage: 128,
                }],
            }],
        };
        let selection = Selection::Rect(Rect::new(3, 2, 2, 1).unwrap());
        let result = execute(&base, selection, &batch).unwrap();
        assert_eq!(
            result.doc.layers[&id].raster.get(1, 1),
            Rgba([200, 100, 50, 64])
        );
        assert_eq!(
            result.doc.layers[&id].raster.get(2, 1),
            Rgba([200, 100, 50, 64])
        );
        assert_eq!(result.doc.layers[&id].raster.get(0, 1), Rgba::TRANSPARENT);
        assert_eq!(result.doc.layers[&id].raster.get(3, 1), Rgba::TRANSPARENT);
        assert_eq!(base.layers[&id].raster.get(1, 1), Rgba::TRANSPARENT);
        assert!(execute_with_budget(&base, selection, &batch, 0).is_err());
        assert_eq!(base.layers[&id].raster.get(1, 1), Rgba::TRANSPARENT);
    }
}
