use crate::WebApp;
use eframe::egui::{Event, Modifiers, PointerButton, Role, Vec2};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_core::types::Rgba;
use paintcraft_tools::Action;

#[test]
fn crop_button_is_selection_gated_undoable_and_preserves_pixels_without_enabling_unimplemented_view()
 {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    assert!(
        h.get_by_role_and_label(Role::Button, "Crop")
            .accesskit_node()
            .is_disabled()
    );
    let layer = h.state().editor.target.unwrap();
    h.state_mut()
        .editor
        .apply(Action::Stroke(paintcraft_tools::Stroke {
            media: Default::default(),
            layer,
            points: vec![(40, 50)],
            point_scale: 1,
            color: Rgba([20, 40, 80, 0]),
            size: 1,
            round: false,
            mode: paintcraft_tools::BlendMode::Replace,
        }))
        .unwrap();
    h.get_by_role_and_label(Role::Button, "Select").click();
    h.run();
    let rect = h.state().canvas.rect.unwrap();
    let a = rect.min + Vec2::new(30.5, 40.5) * h.state().zoom;
    let b = rect.min + Vec2::new(59.5, 79.5) * h.state().zoom;
    h.input_mut().events.extend([
        Event::PointerMoved(a),
        Event::PointerButton {
            pos: a,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        Event::PointerMoved(b),
        Event::PointerButton {
            pos: b,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ]);
    h.run();
    let before = h.state().editor.session.snapshot();
    let selection = h.state().editor.session.selection();
    assert!(h.state().context().can_crop_selection && !h.state().context().can_fit_selection);
    h.get_by_role_and_label(Role::Button, "Crop").click();
    h.run();
    let doc = h.state().editor.session.document();
    assert_eq!(doc.canvas, (30, 40));
    assert_eq!(h.state().preferences.canvas.last, Some((30, 40)));
    assert_eq!(doc.layers[&layer].raster.get(10, 10), Rgba([20, 40, 80, 0]));
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor.session.document().same_semantics(&before));
    assert_eq!(h.state().editor.session.selection(), selection);
    assert_eq!(h.state().preferences.canvas.last, Some((30, 40)));
}

#[test]
fn image_transform_controls_choose_selected_pixels_or_whole_canvas_and_keep_authored_size() {
    use paintcraft_core::types::orthogonal::OrthogonalTransform as Transform;
    use paintcraft_editor::execute::Selection;
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    h.state_mut().edit(Action::ResizeCanvas {
        width: 32,
        height: 24,
    });
    h.run();
    let layer = h.state().editor.target.unwrap();
    h.state_mut().edit(Action::Stroke(paintcraft_tools::Stroke {
        media: Default::default(),
        layer,
        points: vec![(5, 6)],
        point_scale: 1,
        color: Rgba([17, 33, 55, 0]),
        size: 1,
        round: false,
        mode: paintcraft_tools::BlendMode::Replace,
    }));
    h.state_mut().edit(Action::Select {
        corners: Some(((4, 5), (9, 7))),
    });
    h.run();
    let before = h.state().editor.session.snapshot();
    let selection = h.state().editor.session.selection();
    assert_eq!(
        before.layers[&layer].raster.get(5, 6),
        Rgba([17, 33, 55, 0]),
        "{}",
        h.state().message
    );
    assert_eq!(
        selection,
        Selection::Rect(paintcraft_core::types::Rect::new(4, 5, 6, 3).unwrap())
    );
    h.get_by_role_and_label(Role::Button, "Flip").click();
    h.run();
    h.get_by_label("Flip horizontal").click();
    h.run();
    assert!(h.query_by_label("Flip horizontal").is_none());
    let doc = h.state().editor.session.document();
    assert_eq!(doc.canvas, (32, 24));
    assert_eq!(
        doc.layers[&layer].raster.get(8, 6),
        Rgba([17, 33, 55, 0]),
        "{} selection {:?}",
        h.state().message,
        h.state().editor.session.selection()
    );
    h.get_by_label("Undo").click();
    h.run();
    assert!(h.state().editor.session.document().same_semantics(&before));
    assert_eq!(h.state().editor.session.selection(), selection);
    h.state_mut().edit(Action::Select { corners: None });
    h.run();
    h.get_by_role_and_label(Role::Button, "Rotate").click();
    h.run();
    h.get_by_label("Rotate right 90°").click();
    h.run();
    assert_eq!(h.state().editor.session.document().canvas, (24, 32));
    assert_eq!(h.state().preferences.canvas.last, Some((24, 32)));
    assert_eq!(h.state().editor.session.selection(), Selection::Inactive);
    let l = &h.state().editor.session.document().layers[&layer];
    assert_eq!(
        l.raster.get(17 - l.origin.0, 5 - l.origin.1),
        Rgba([17, 33, 55, 0])
    );
    h.state_mut().edit(Action::Undo);
    h.run();
    assert_eq!(h.state().preferences.canvas.last, Some((24, 32)));
    h.state_mut().edit(Action::TransformImage {
        transform: Transform::FlipVertical,
        clear: Rgba([255; 4]),
    });
    h.run();
    let l = &h.state().editor.session.document().layers[&layer];
    assert_eq!(
        l.raster.get(5 - l.origin.0, 17 - l.origin.1),
        Rgba([17, 33, 55, 0])
    );
    assert!(h.state().message.starts_with("Unsaved"));
}

#[test]
fn historical_rotate_popup_maps_pixels_and_undoes_in_both_languages() {
    use paintcraft_classic::ClassicVariant;
    use paintcraft_ui::i18n::{Language, set_language, translate};
    for variant in [ClassicVariant::V7, ClassicVariant::V10] {
        for language in Language::ALL {
            let mut h = Harness::builder()
                .with_size(Vec2::new(1360.0, 900.0))
                .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
            h.state_mut().variant = variant;
            set_language(&h.ctx, language);
            h.state_mut().edit(Action::ResizeCanvas {
                width: 32,
                height: 24,
            });
            let layer = h.state().editor.target.unwrap();
            h.state_mut().edit(Action::Stroke(paintcraft_tools::Stroke {
                media: Default::default(),
                layer,
                points: vec![(5, 5)],
                point_scale: 1,
                color: Rgba([17, 33, 55, 0]),
                size: 1,
                round: false,
                mode: paintcraft_tools::BlendMode::Replace,
            }));
            h.run();
            let label = |s| translate(language, s);
            for selected in [false, true] {
                if selected {
                    h.state_mut().edit(Action::Select {
                        corners: Some(((4, 5), (9, 7))),
                    });
                    h.run();
                }
                let source = h.state().editor.session.snapshot();
                let selection = h.state().editor.session.selection();
                for (name, whole, region, swap) in [
                    ("Rotate right 90°", (18, 5), (6, 6), true),
                    ("Rotate left 90°", (5, 26), (4, 9), true),
                    ("Rotate 180°", (26, 18), (8, 7), false),
                    ("Flip vertical", (5, 18), (5, 7), false),
                    ("Flip horizontal", (26, 5), (8, 5), false),
                ] {
                    let before = h.state().editor.session.undo_depth();
                    h.get_by_role_and_label(Role::Button, &label("Rotate"))
                        .click();
                    h.run();
                    let caption = label(name);
                    let row = h.get_by_role_and_label(Role::Button, &caption);
                    assert!(row.rect().height() <= 26.0);
                    row.click();
                    h.run();
                    assert!(h.query_by_role_and_label(Role::Button, &caption).is_none());
                    assert_eq!(h.state().editor.session.undo_depth(), before + 1);
                    let doc = h.state().editor.session.document();
                    let (x, y) = if selected { region } else { whole };
                    assert_eq!(
                        doc.layers[&layer].raster.get(
                            x - doc.layers[&layer].origin.0,
                            y - doc.layers[&layer].origin.1
                        ),
                        Rgba([17, 33, 55, 0]),
                        "{variant:?} {name} selected={selected}"
                    );
                    assert_eq!(
                        doc.canvas,
                        if swap && !selected {
                            (24, 32)
                        } else {
                            (32, 24)
                        }
                    );
                    h.get_by_role_and_label(Role::Button, &label("Undo"))
                        .click();
                    h.run();
                    assert!(h.state().editor.session.document().same_semantics(&source));
                    assert_eq!(h.state().editor.session.selection(), selection);
                }
            }
        }
    }
}
