//! The flat render baseline of `paintcraft.core/default-v1`.
//!
//! Start from a canvas-sized buffer of transparent black. For each visible root
//! layer, bottom to top, apply layer opacity to every sample inside the layer's
//! bounds and composite it with straight RGBA8 source-over. Output is straight
//! RGBA8 encoded sRGB, row-major.

use paintcraft_core::types::{Error, ErrorCode, Result, Rgba};

use paintcraft_document::Document;

/// Largest canvas this renderer will materialize (provisional admission limit).
pub const RENDER_PIXELS_MAX: u64 = 67_108_864;

pub fn render_flat(doc: &Document) -> Result<Vec<u8>> {
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
    let mut span = Vec::new();
    for layer in doc.layers_in_order() {
        if !layer.visible || layer.opacity == 0 {
            continue;
        }
        let Some(area) = canvas.intersect(&layer.extent()) else {
            continue;
        };
        span.resize(area.width() as usize * 4, 0);
        for y in area.y0..area.y1 {
            layer
                .raster
                .read_span(y - layer.origin.1, area.x0 - layer.origin.0, &mut span);
            let row = &mut out[(y as usize * width + area.x0 as usize) * 4..][..span.len()];
            for (dst, src) in row
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(span.as_chunks::<4>().0)
            {
                *dst = Rgba(*src)
                    .with_opacity(layer.opacity)
                    .source_over(Rgba(*dst))
                    .0;
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
    fn oversized_canvas_is_refused() {
        let doc = new_document(16_384, 16_384, None, 0, POLICY).unwrap();
        assert_eq!(
            render_flat(&doc).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
}
