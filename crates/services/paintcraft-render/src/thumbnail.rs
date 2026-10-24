//! Bounded full-document and isolated-layer previews.
use crate::{
    display::{DisplayFrame, DisplayRequest, render_display_cancellable},
    *,
};
use paintcraft_document::Layer;

/// Compose the full canvas before box filtering, preserving ordinary layer order,
/// visibility and opacity. Output is bounded; dense source work still belongs on
/// an admitted worker. This never creates a full-canvas intermediate image.
pub fn render_document(
    doc: &Document,
    side: u16,
    check: impl Fn() -> Result<()>,
) -> Result<DisplayFrame> {
    check()?;
    if !(1..=256).contains(&side) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "thumbnail side must be 1..=256",
        ));
    }
    let mut level = 0;
    while doc.canvas.0.max(doc.canvas.1).div_ceil(1 << level) > u32::from(side) {
        level += 1;
    }
    let request = DisplayRequest::new(doc, doc.canvas_rect(), level)?;
    render_display_cancellable(doc, request, check)
}

/// Render one layer against transparency, positioned in the document canvas.
/// Output is at most `side` samples per dimension; no full-canvas buffer is made.
/// Box filtering and alpha rounding use the ordinary display compositor.
/// Call off the UI thread: output is bounded, but dense source work can be large.
pub fn render_layer(
    doc: &Document,
    layer: &Layer,
    side: u16,
    check: impl Fn() -> Result<()>,
) -> Result<DisplayFrame> {
    check()?;
    if !(1..=128).contains(&side) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "thumbnail side must be 1..=128",
        ));
    }
    let mut isolated = layer.clone();
    isolated.visible = true;
    isolated.opacity = 255;
    let isolated = Document {
        document_id: doc.document_id,
        state_id: doc.state_id,
        canvas: doc.canvas,
        roots: vec![layer.id],
        layers: [(layer.id, isolated)].into(),
        title: None,
    };
    render_document(&isolated, side, check)
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_document::new_document;
    use std::cell::Cell;

    #[test]
    fn document_preview_composes_order_visibility_and_opacity_before_filtering() {
        let mut doc = new_document(
            5,
            3,
            Some(Rgba([255, 0, 0, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let mut top = doc.layers[&doc.roots[0]].clone();
        top.id = "00000000-0000-0000-0000-000000000999".parse().unwrap();
        let top_id = top.id;
        top.opacity = 128;
        top.raster = paintcraft_raster::RasterLayer::new(5, 3, Rgba([0, 0, 255, 255]));
        doc.roots.push(top.id);
        doc.layers.insert(top.id, top);
        let before = doc.clone();
        let frame = render_document(&doc, 3, || Ok(())).unwrap();
        assert_eq!(frame.request.size(), [3, 2]);
        assert_eq!(frame.pixels, [127, 0, 128, 255].repeat(6));
        assert!(doc.same_semantics(&before));
        doc.layers.get_mut(&top_id).unwrap().visible = false;
        let frame = render_document(&doc, 3, || Ok(())).unwrap();
        assert_eq!(frame.pixels, [255, 0, 0, 255].repeat(6));
    }

    #[test]
    fn huge_document_preview_is_bounded_and_cancellation_returns_no_frame() {
        let doc = new_document(
            1_000_000,
            800_000,
            Some(Rgba([5, 40, 80, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let frame = render_document(&doc, 256, || Ok(())).unwrap();
        let [width, height] = frame.request.size();
        assert!(width <= 256 && height <= 256);
        assert_eq!(frame.pixels.len(), width as usize * height as usize * 4);
        assert_eq!(frame.stats.native_pixels, 0);
        assert!(
            frame
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| *pixel == [5, 40, 80, 255])
        );
        for side in [0, 257] {
            assert_eq!(
                render_document(&doc, side, || Ok(())).unwrap_err().code,
                ErrorCode::InputSchema
            );
        }
        assert_eq!(
            render_document(&doc, 256, || Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel preview"
            )))
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
    }

    #[test]
    fn hidden_translucent_layer_keeps_raw_content_and_does_not_change_document() {
        let mut doc = new_document(
            5,
            3,
            Some(Rgba([20, 40, 80, 100])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let id = doc.roots[0];
        doc.layers.get_mut(&id).unwrap().visible = false;
        doc.layers.get_mut(&id).unwrap().opacity = 0;
        let before = doc.clone();
        let frame = render_layer(&doc, &doc.layers[&id], 3, || Ok(())).unwrap();
        assert_eq!(frame.request.size(), [3, 2]);
        assert_eq!(frame.pixels, [20, 40, 80, 100].repeat(6));
        assert!(doc.same_semantics(&before));
    }

    #[test]
    fn isolated_offset_layer_filters_clipped_alpha_without_other_layers() {
        let doc = new_document(5, 3, Some(Rgba([255; 4])), 0, paintcraft_document::POLICY).unwrap();
        let mut layer = doc.layers[&doc.roots[0]].clone();
        layer.visible = false;
        layer.opacity = 0;
        layer.origin = (-1, 1);
        layer.raster = paintcraft_raster::RasterLayer::from_rgba(
            2,
            2,
            Rgba::TRANSPARENT,
            &[
                0, 255, 0, 255, 255, 0, 0, 128, 255, 255, 0, 255, 0, 0, 255, 64,
            ],
            &mut paintcraft_raster::Admission::new(65536),
        )
        .unwrap();
        let frame = render_layer(&doc, &layer, 3, || Ok(())).unwrap();
        assert_eq!(frame.request.size(), [3, 2]);
        // One red half-alpha source among four canvas pixels, and one blue
        // quarter-alpha source among two pixels at the partial bottom edge.
        assert_eq!(
            frame.pixels,
            [
                255, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 32, 0, 0, 0, 0, 0, 0, 0, 0,
            ]
        );
    }

    #[test]
    fn million_square_uniform_thumbnail_has_bounded_output_and_no_native_intermediate() {
        let doc = new_document(
            1_000_000,
            1_000_000,
            Some(Rgba([255; 4])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let frame = render_layer(&doc, &doc.layers[&doc.roots[0]], 64, || Ok(())).unwrap();
        assert!(frame.request.size().into_iter().all(|side| side <= 64));
        assert_eq!(frame.stats.native_pixels, 0);
        assert!(frame.pixels.len() <= 64 * 64 * 4);
        assert!(
            frame
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| *pixel == [255; 4])
        );
    }

    #[test]
    fn invalid_size_and_cancellation_never_return_partial_previews() {
        let doc = new_document(5, 3, Some(Rgba([255; 4])), 0, paintcraft_document::POLICY).unwrap();
        let layer = &doc.layers[&doc.roots[0]];
        for side in [0, 129] {
            assert_eq!(
                render_layer(&doc, layer, side, || Ok(())).unwrap_err().code,
                ErrorCode::InputSchema
            );
        }
        let checks = Cell::new(0);
        let error = render_layer(&doc, layer, 64, || {
            checks.set(checks.get() + 1);
            if checks.get() > 2 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel thumbnail",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
}
