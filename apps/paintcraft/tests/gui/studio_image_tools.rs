use super::*;
use paintcraft_core::types::orthogonal::OrthogonalTransform;
use paintcraft_tools::{ShapePreset, Tool};
#[test]
fn studio_exposes_every_shape_and_image_transforms_without_classic_chrome() {
    let (mut h, dir) = super::studio::studio_harness(true);
    h.set_size(Vec2::new(1480.0, 960.0));
    h.run();
    h.get_by_role_and_label(eframe::egui::Role::Button, "Rectangle")
        .click();
    h.run();
    for preset in ShapePreset::ALL {
        h.get_by_label("Shape").click();
        h.run();
        h.get_by_label(preset.name()).scroll_to_me();
        h.run();
        h.get_by_label(preset.name()).click();
        h.run();
        assert_eq!(h.state().editor().tool, Tool::Preset(preset));
    }
    let before = h.state().editor().document_snapshot();
    h.get_by_role_and_label(eframe::egui::Role::Button, "Image")
        .click();
    h.run();
    assert!(h.get_by_label("Crop").accesskit_node().is_disabled());
    for transform in [
        OrthogonalTransform::RotateRight,
        OrthogonalTransform::RotateLeft,
        OrthogonalTransform::Rotate180,
        OrthogonalTransform::FlipHorizontal,
        OrthogonalTransform::FlipVertical,
    ] {
        let label = paintcraft_ui::actions::Action::TransformImage(transform)
            .descriptor()
            .label;
        assert!(!h.get_by_label(label).accesskit_node().is_disabled());
    }
    h.get_by_role_and_label(eframe::egui::Role::Button, "Image")
        .get_by_label("Clear Canvas")
        .click();
    h.run();
    assert_eq!(h.state().editor().document().canvas, before.canvas);
    let id = h.state().editor().target().unwrap();
    assert_eq!(
        h.state().editor().document().layers[&id].raster.get(0, 0),
        Rgba::TRANSPARENT
    );
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&before));
    h.get_by_role_and_label(eframe::egui::Role::Button, "Image")
        .click();
    h.run();
    h.get_by_label("Rotate right 90°").click();
    h.run();
    assert_eq!(
        h.state().editor().document().canvas,
        (before.canvas.1, before.canvas.0)
    );
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&before));
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
