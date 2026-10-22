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
        paintcraft_commands::validate_mask_spans_checked(spans, self.check)?;
        for (index, span) in spans.iter().enumerate() {
            if index.is_multiple_of(1024) {
                (self.check)()?;
            }
            self.pencil_pixels += i128::from(span.x1) - i128::from(span.x0) + 1;
        }
        if self.pencil_pixels > PENCIL_PIXELS_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "mask pixel budget exceeded",
            ));
        }
        let id = self.resolve(layer)?;
        let mut plan = self.adm.paint_plan();
        if let Some(clip) = self.editable(&self.doc.layers[&id]) {
            for (index, span) in spans.iter().enumerate() {
                if index.is_multiple_of(1024) {
                    (self.check)()?;
                }
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
