use super::*;
use eframe::egui::Role;
use paintcraft_tools::Tool;
fn click_canvas(h: &mut Harness<App>, x: f32, y: f32, button: eframe::egui::PointerButton) {
    let rect = h.state().canvas_rect().unwrap();
    let size = h.state().editor().document().canvas;
    let p = rect.min
        + Vec2::new(
            (x + 0.5) * rect.width() / size.0 as f32,
            (y + 0.5) * rect.height() / size.1 as f32,
        );
    h.input_mut().events.extend([
        Event::PointerMoved(p),
        Event::PointerButton {
            pos: p,
            button,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        Event::PointerButton {
            pos: p,
            button,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ]);
    h.run();
}
#[test]
fn classic_picker_samples_primary_or_secondary_returns_to_previous_tool_and_never_edits() {
    for ppp in [1.0, 2.0] {
        let mut h = harness_with(App::new(None), ppp, true);
        h.get_by_label("Color #ED1C24").click();
        h.run();
        click_canvas(&mut h, 40.0, 50.0, eframe::egui::PointerButton::Primary);
        assert_eq!(top_pixel(&h, 40, 50), Rgba([237, 28, 36, 255]));
        h.get_by_label("Color #3F48CC").click();
        h.run();
        let before = h.state().editor().document_snapshot();
        let ticket = h.state().editor().history_view().ticket;
        let count = publications(&h);
        let selection = h.state().editor().selection();
        let secondary = h.state().editor().secondary;
        h.get_by_role_and_label(Role::Button, "Color picker")
            .click();
        h.run();
        click_canvas(&mut h, 40.0, 50.0, eframe::egui::PointerButton::Primary);
        assert_eq!(h.state().editor().color, Rgba([237, 28, 36, 255]));
        assert_eq!(h.state().editor().secondary, secondary);
        assert_eq!(h.state().editor().tool, Tool::Pencil);
        h.get_by_role_and_label(Role::Button, "Color picker")
            .click();
        h.run();
        click_canvas(&mut h, 40.0, 50.0, eframe::egui::PointerButton::Secondary);
        assert_eq!(h.state().editor().secondary, Rgba([237, 28, 36, 255]));
        assert_eq!(h.state().editor().tool, Tool::Pencil);
        h.get_by_role_and_label(Role::Button, "Color picker")
            .click();
        h.run();
        click_canvas(&mut h, 70.0, 60.0, eframe::egui::PointerButton::Primary);
        assert_eq!(h.state().editor().color, Rgba([255; 4]));
        assert!(std::sync::Arc::ptr_eq(
            &before,
            &h.state().editor().document_snapshot()
        ));
        assert_eq!(publications(&h), count);
        assert_eq!(h.state().editor().history_view().ticket, ticket);
        assert_eq!(h.state().editor().selection(), selection);
    }
}

#[test]
fn studio_picker_exposes_sampling_choices_raw_hidden_layer_ctrl_image_and_after_action() {
    use paintcraft::settings::{Experience, Settings};
    use paintcraft_core::types::picker::{SampleSize, SampleSource};
    use paintcraft_document::{Layer, POLICY, new_document};
    use paintcraft_raster::RasterLayer;
    let dir = scratch("studio-picker");
    Settings {
        experience: Experience::Studio,
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let mut doc = new_document(320, 320, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let red = uuid::Uuid::new_v4();
    doc.roots.push(red);
    doc.layers.insert(
        red,
        Layer {
            id: red,
            name: "Red overlay".into(),
            origin: (100, 100),
            visible: true,
            opacity: 128,
            raster: RasterLayer::new(160, 160, Rgba([255, 0, 0, 128])),
        },
    );
    let hidden = uuid::Uuid::new_v4();
    doc.roots.push(hidden);
    doc.layers.insert(
        hidden,
        Layer {
            id: hidden,
            name: "Hidden raw".into(),
            origin: (100, 100),
            visible: false,
            opacity: 0,
            raster: RasterLayer::new(160, 160, Rgba([17, 33, 55, 0])),
        },
    );
    let path = dir.join("source.pcnd");
    std::fs::write(&path, paintcraft_formats::pcnd::write(&doc).unwrap()).unwrap();
    let mut h = harness_with(App::new(Some(path)).with_settings(dir.clone()), 1.0, true);
    h.get_by_role_and_label(Role::Button, "Color picker")
        .click();
    h.run();
    let before = h.state().editor().document_snapshot();
    let ticket = h.state().editor().history_view().ticket;
    assert_eq!(h.state().editor().target(), Some(hidden));
    click_canvas(&mut h, 160.0, 160.0, PointerButton::Primary);
    assert_eq!(h.state().editor().color, Rgba([17, 33, 55, 0]));
    assert_eq!(h.state().editor().tool, Tool::ColorPicker);
    h.get_by_label("Sample size").click();
    h.run();
    h.get_by_label("51 × 51").click();
    h.run();
    assert_eq!(h.state().editor().options.picker_size, SampleSize::FiftyOne);
    let p = at(&h, 160.0, 160.0);
    h.input_mut().events.extend([
        Event::PointerMoved(p),
        Event::PointerButton {
            pos: p,
            button: PointerButton::Secondary,
            pressed: true,
            modifiers: Modifiers::CTRL,
        },
        Event::PointerButton {
            pos: p,
            button: PointerButton::Secondary,
            pressed: false,
            modifiers: Modifiers::CTRL,
        },
    ]);
    h.run();
    assert_eq!(h.state().editor().secondary, Rgba([255, 191, 191, 255]));
    assert_eq!(
        h.state().editor().options.picker_source,
        SampleSource::Layer
    );
    h.get_by_label("Sampling source").click();
    h.run();
    h.query_all_by_role_and_label(Role::Button, "Image")
        .find(|node| node.accesskit_node().toggled().is_some())
        .expect("the sampling option, rather than the Image menu")
        .click();
    h.run();
    assert_eq!(
        h.state().editor().options.picker_source,
        SampleSource::Image
    );
    h.get_by_label("After picking").click();
    h.run();
    h.get_by_label("Switch to Pencil").click();
    h.run();
    click_canvas(&mut h, 160.0, 160.0, PointerButton::Primary);
    assert_eq!(h.state().editor().color, Rgba([255, 191, 191, 255]));
    assert_eq!(h.state().editor().tool, Tool::Pencil);
    assert_eq!(h.state().editor().history_view().ticket, ticket);
    assert!(std::sync::Arc::ptr_eq(
        &before,
        &h.state().editor().document_snapshot()
    ));
    assert!(!h.state().editor().is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}
