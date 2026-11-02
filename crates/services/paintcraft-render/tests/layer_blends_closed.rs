//! First metadata wave refuses appearance rather than silently treating modes as Normal.
use paintcraft_core::types::{ErrorCode, LayerBlendMode, Rgba};
use std::cell::Cell;
#[test]
fn all_stored_non_normal_intents_refuse_before_renderer_callbacks() {
    for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        for (visible, opacity) in [(true, 255), (false, 255), (true, 0)] {
            let mut doc = paintcraft_document::new_document(
                1,
                1,
                Some(Rgba([17, 32, 255, 0])),
                0,
                paintcraft_document::POLICY,
            )
            .unwrap();
            let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
            layer.blend_mode = mode;
            layer.visible = visible;
            layer.opacity = opacity;
            let checks = Cell::new(0);
            let result = paintcraft_render::render_cancellable(&doc, || {
                checks.set(checks.get() + 1);
                Ok(())
            });
            assert_eq!(result.unwrap_err().code, ErrorCode::CapabilityUnsupported);
            assert_eq!(checks.get(), 0);
        }
    }
}
