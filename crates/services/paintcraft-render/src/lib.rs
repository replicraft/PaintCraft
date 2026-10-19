//! The flat render baseline of `paintcraft.core/default-v1`.
//!
//! Start from a canvas-sized buffer of transparent black. For each visible root
//! layer, bottom to top, apply layer opacity to every sample inside the layer's
//! bounds and composite it with straight RGBA8 source-over. Output is straight
//! RGBA8 encoded sRGB, row-major.

use paintcraft_core::types::{Error, ErrorCode, Result, Rgba};

use paintcraft_document::Document;
use paintcraft_raster::RowSpan;

/// Largest canvas this renderer will materialize (provisional admission limit).
pub const RENDER_PIXELS_MAX: u64 = 67_108_864;

pub fn render_flat(doc: &Document) -> Result<Vec<u8>> {
    render_cancellable(doc, || Ok(()))
}

pub fn render_cancellable(doc: &Document, check: impl Fn() -> Result<()>) -> Result<Vec<u8>> {
    check()?;
    let (w, h) = (u64::from(doc.canvas.0), u64::from(doc.canvas.1));
    if w * h > RENDER_PIXELS_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "canvas exceeds the render admission limit",
        ));
    }
    let width = w as usize;
    let mut out = vec![0u8; width * h as usize * 4];
    let canvas = doc.canvas_rect();
    for layer in doc.layers_in_order() {
        if !layer.visible || layer.opacity == 0 {
            continue;
        }
        let Some(area) = canvas.intersect(&layer.extent()) else {
            continue;
        };
        for y in area.y0..area.y1 {
            check()?;
            let mut row = &mut out[(y as usize * width + area.x0 as usize) * 4..]
                [..area.width() as usize * 4];
            for span in layer.raster.row_spans(
                (y - layer.origin.1) as u32,
                (area.x0 - layer.origin.0) as u32,
                area.width() as u32,
            )? {
                let (dst, rest) = row.split_at_mut(span.pixel_count() * 4);
                row = rest;
                match span {
                    RowSpan::Uniform { color, .. } => {
                        let color = color.with_opacity(layer.opacity);
                        match color.a() {
                            0 => {}
                            255 => dst.as_chunks_mut::<4>().0.fill(color.0),
                            _ => {
                                for pixel in dst.as_chunks_mut::<4>().0 {
                                    *pixel = color.source_over(Rgba(*pixel)).0;
                                }
                            }
                        }
                    }
                    RowSpan::Pixels(src) => {
                        for (dst, src) in dst
                            .as_chunks_mut::<4>()
                            .0
                            .iter_mut()
                            .zip(src.as_chunks::<4>().0)
                        {
                            let color = Rgba(*src).with_opacity(layer.opacity);
                            match color.a() {
                                0 => {}
                                255 => *dst = color.0,
                                _ => *dst = color.source_over(Rgba(*dst)).0,
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_document::{POLICY, new_document};
    use paintcraft_editor::session::Session;

    fn run(s: &mut Session, ops: &str) {
        let t = s.ticket();
        let text = format!(
            r#"{{"schema":"paintcraft.batch/v1","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":{ops}}}"#,
            t.state_id, t.publication_epoch
        );
        s.apply_json(text.as_bytes()).unwrap();
    }

    #[test]
    fn no_layers_renders_transparent_black() {
        let s = Session::new(new_document(3, 2, None, 0, POLICY).unwrap());
        assert_eq!(render_flat(s.document()).unwrap(), vec![0u8; 24]);
    }

    #[test]
    fn opacity_visibility_and_clipping() {
        let mut s =
            Session::new(new_document(3, 1, Some(Rgba([255, 255, 255, 255])), 0, POLICY).unwrap());
        run(
            &mut s,
            r##"[{"op":"layer.add","ref":"ink","name":"Ink","width":4,"height":1,"origin":{"x":-1,"y":0},"default_pixel":"#FF000080"},
                {"op":"layer.set_properties","layer_ref":{"ref":"ink"},"opacity_u8":128}]"##,
        );
        // Layer pixel (255,0,0,128) at opacity 128 has alpha 64 over opaque white.
        let red64 = Rgba([255, 0, 0, 64])
            .source_over(Rgba([255, 255, 255, 255]))
            .0;
        let img = render_flat(s.document()).unwrap();
        assert_eq!(img.as_chunks::<4>().0, [red64, red64, red64]);
        let ink = s.document().roots[1];
        run(
            &mut s,
            &format!(
                r#"[{{"op":"layer.set_properties","layer_ref":{{"id":"{ink}"}},"visible":false}}]"#
            ),
        );
        assert!(
            render_flat(s.document())
                .unwrap()
                .chunks(4)
                .all(|p| p == [255; 4])
        );
    }

    #[test]
    fn cancelled_render_never_returns_a_partial_composite() {
        let doc = new_document(100, 100, Some(Rgba([255; 4])), 0, POLICY).unwrap();
        let checks = std::cell::Cell::new(0);
        let result = render_cancellable(&doc, || {
            checks.set(checks.get() + 1);
            if checks.get() >= 3 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "test cancellation",
                ))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(checks.get(), 3);
        assert_eq!(render_flat(&doc).unwrap(), vec![255; 100 * 100 * 4]);
    }

    #[test]
    fn oversized_canvas_is_refused() {
        let doc = new_document(16_384, 16_384, None, 0, POLICY).unwrap();
        assert_eq!(
            render_flat(&doc).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
}

#[cfg(feature = "gpu")]
pub mod gpu;
