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
                self.paint_spans(id, &[Span { y: span.y, x0, x1 }], pixel, Mode::SourceOver)?;
            }
        }
        Ok(())
    }
}
