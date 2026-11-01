use crate::WebApp;
use eframe::egui::{Event, Modifiers, PointerButton, Role, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_core::types::Rgba;
use paintcraft_tools::{Action, Tool};
pub(crate) fn option_fixture() -> paintcraft_document::Document {
    use paintcraft_core::types::filters::Descriptor;
    use paintcraft_document::{FilterInstance, FilterStack};
    let mut rgba = [10, 20, 30, 255].repeat(81);
    rgba[(4 * 9 + 4) * 4..(4 * 9 + 5) * 4].copy_from_slice(&[200, 100, 50, 255]);
    let mut doc = paintcraft_document::import_image(9, 9, &rgba, "picker-options").unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            uuid::Uuid::from_u128(123),
            true,
            Descriptor::InvertRgbV1,
        )])
        .unwrap();
    doc
}

fn option_harness(frontend: paintcraft_ui::actions::Frontend) -> Harness<'static, WebApp> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.state_mut().editor = crate::editor::Editor::from_document(option_fixture()).unwrap();
    h.state_mut().frontend = frontend;
    h.state_mut().controls.tool = Tool::Pencil;
    h.run();
    h
}

fn option_pointer(h: &mut Harness<WebApp>, down: bool, ctrl: bool) {
    let pos = h.state().canvas.rect.unwrap().min + Vec2::splat(4.5) * h.state().zoom;
    let modifiers = Modifiers {
        ctrl,
        command: ctrl,
        ..Modifiers::NONE
    };
    h.input_mut().events.extend([
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: down,
            modifiers,
        },
    ]);
    h.run_steps(1);
}

#[test]
fn studio_canvas_honors_layer_image_three_and_after_behavior_without_history() {
    use paintcraft_core::types::picker::{PickerAfter, SampleSize, SampleSource};
    use paintcraft_ui::actions::Frontend;
    for (source, after, expected, tool) in [
        (
            SampleSource::Layer,
            PickerAfter::Stay,
            [31, 29, 32, 255],
            Tool::ColorPicker,
        ),
        (
            SampleSource::Image,
            PickerAfter::Pencil,
            [224, 226, 223, 255],
            Tool::Pencil,
        ),
    ] {
        let mut h = option_harness(Frontend::Studio);
        h.state_mut().controls.tool = Tool::ColorPicker;
        h.state_mut().controls.options.picker_source = source;
        h.state_mut().controls.options.picker_size = SampleSize::Three;
        h.state_mut().controls.options.picker_after = after;
        h.run();
        let before = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.ticket();
        option_pointer(&mut h, true, false);
        option_pointer(&mut h, false, false);
        assert_eq!(h.state().controls.primary, Rgba(expected));
        assert_eq!(h.state().controls.tool, tool);
        assert_eq!(h.state().editor.session.ticket(), ticket);
        assert!(std::sync::Arc::ptr_eq(
            &before,
            &h.state().editor.session.snapshot()
        ));
        assert!(!h.state().editor.session.is_dirty());
    }
}

#[test]
fn classic_canvas_retains_image_one_previous_despite_studio_preferences() {
    use paintcraft_core::types::picker::{PickerAfter, SampleSize, SampleSource};
    let mut h = option_harness(paintcraft_ui::actions::Frontend::Classic);
    h.state_mut().controls.tool = Tool::ColorPicker;
    h.state_mut().controls.options.picker_source = SampleSource::Layer;
    h.state_mut().controls.options.picker_size = SampleSize::Three;
    h.state_mut().controls.options.picker_after = PickerAfter::Stay;
    h.run();
    let ticket = h.state().editor.session.ticket();
    option_pointer(&mut h, true, false);
    option_pointer(&mut h, false, false);
    assert_eq!(h.state().controls.primary, Rgba([55, 155, 205, 255]));
    assert_eq!(h.state().controls.tool, Tool::Pencil);
    assert_eq!(h.state().editor.session.ticket(), ticket);
}

#[test]
fn studio_ctrl_override_is_captured_once_and_preferences_survive_ctrl_release() {
    use paintcraft_core::types::picker::{PickerAfter, SampleSize, SampleSource};
    let mut h = option_harness(paintcraft_ui::actions::Frontend::Studio);
    h.state_mut().controls.tool = Tool::ColorPicker;
    h.state_mut().controls.options.picker_source = SampleSource::Layer;
    h.state_mut().controls.options.picker_size = SampleSize::Three;
    h.state_mut().controls.options.picker_after = PickerAfter::Stay;
    h.run();
    let ticket = h.state().editor.session.ticket();
    option_pointer(&mut h, true, true);
    assert_eq!(
        h.state().editor.tools.options.picker_source,
        SampleSource::Layer
    );
    option_pointer(&mut h, false, false);
    assert_eq!(h.state().controls.primary, Rgba([224, 226, 223, 255]));
    assert_eq!(
        h.state().controls.options.picker_source,
        SampleSource::Layer
    );
    assert_eq!(h.state().controls.options.picker_size, SampleSize::Three);
    assert_eq!(h.state().controls.tool, Tool::ColorPicker);
    option_pointer(&mut h, true, false);
    option_pointer(&mut h, false, false);
    assert_eq!(h.state().controls.primary, Rgba([31, 29, 32, 255]));
    assert_eq!(h.state().editor.session.ticket(), ticket);
}
fn click(h: &mut Harness<WebApp>, x: f32, y: f32, button: PointerButton) {
    let p = h.state().canvas.rect.unwrap().min + Vec2::new(x + 0.5, y + 0.5) * h.state().zoom;
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
fn web_picker_uses_real_document_pixels_and_updates_controls_without_history_or_dirtiness() {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1360.0, 900.0))
        .build_eframe(|cc| WebApp::new(&cc.egui_ctx));
    h.run();
    let clean = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.ticket();
    h.get_by_role_and_label(Role::Button, "Color picker")
        .click();
    h.run();
    click(&mut h, 40.0, 50.0, PointerButton::Primary);
    assert_eq!(h.state().controls.primary, Rgba([255; 4]));
    assert_eq!(h.state().controls.tool, Tool::Brush);
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert!(std::sync::Arc::ptr_eq(
        &clean,
        &h.state().editor.session.snapshot()
    ));
    assert!(!h.state().editor.session.is_dirty());
    h.get_by_role_and_label(Role::Button, "Pencil").click();
    h.run();
    let layer = h.state().editor.target.unwrap();
    h.state_mut().edit(Action::Stroke(paintcraft_tools::Stroke {
        media: Default::default(),
        layer,
        points: vec![(40, 50)],
        point_scale: 1,
        color: Rgba([255, 0, 0, 255]),
        size: 1,
        round: false,
        mode: paintcraft_tools::BlendMode::Replace,
    }));
    h.run();
    h.state_mut().edit(Action::Select {
        corners: Some(((70, 70), (71, 71))),
    });
    h.run();
    let before = h.state().editor.session.snapshot();
    let selection = h.state().editor.session.selection();
    let ticket = h.state().editor.session.ticket();
    h.get_by_role_and_label(Role::Button, "Color picker")
        .click();
    h.run();
    click(&mut h, 40.0, 50.0, PointerButton::Secondary);
    assert_eq!(h.state().controls.secondary, Rgba([255, 0, 0, 255]));
    assert_eq!(
        h.state().editor.tools.secondary,
        h.state().controls.secondary
    );
    assert_eq!(h.state().controls.primary, Rgba([255; 4]));
    assert_eq!(h.state().controls.tool, Tool::Pencil);
    assert_eq!(h.state().editor.session.selection(), selection);
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert!(std::sync::Arc::ptr_eq(
        &before,
        &h.state().editor.session.snapshot()
    ));
}
