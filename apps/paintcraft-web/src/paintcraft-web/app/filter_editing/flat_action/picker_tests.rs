//! Authored publication/geometry controls. No simulated test claims Worker ACKs.
use super::*;
use paintcraft_core::types::{
    Rgba,
    filters::{AddNoiseRgbV1, Descriptor},
    picker::{SampleSize, SampleSource},
};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_tools::{Action, ColorPick, PointerButton};
fn setup() -> (egui::Context, WebApp) {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
    let mut doc = Editor::new(53, 53).unwrap().session.document().clone();
    let root = doc.roots[0];
    doc.layers.get_mut(&root).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(987),
        false,
        Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
    )])
    .unwrap();
    app.editor = Editor::from_document(doc).unwrap();
    app.editor.tools.choose_tool(Tool::ColorPicker);
    (ctx, app)
}
fn pick(app: &WebApp) -> ColorPick {
    ColorPick {
        original: app.editor.tools.color,
        revision: app.editor.tools.preview_revision(),
        layer: app.editor.session.document().roots[0],
        at: (0, 0),
        button: PointerButton::Primary,
        source: SampleSource::Image,
        size: SampleSize::One,
        return_resource_shape: None,
        return_to: None,
    }
}
fn prepared_outcome(app: &mut WebApp, color: Rgba) {
    let mut job = app.filter_display.picker_wanted.take().unwrap();
    job.outcome = Some(Rc::new(RefCell::new(Some(Ok(Output::Picked(color))))));
    app.filter_display.copy_action = Some(job);
}
#[test]
fn sample_geometry_all_sizes_clips_canvas_and_does_not_follow_selection() {
    let (_, app) = setup();
    let mut p = pick(&app);
    for size in SampleSize::ALL {
        p.size = size;
        for at in [(0, 0), (26, 26), (52, 52)] {
            p.at = at;
            let roi = picker::region(app.editor.session.document(), &p).unwrap();
            let radius = i64::from(size.side() / 2);
            assert_eq!(
                (roi.x0, roi.y0),
                ((at.0 - radius).max(0), (at.1 - radius).max(0))
            );
            assert_eq!(
                (roi.x1, roi.y1),
                ((at.0 + radius + 1).min(53), (at.1 + radius + 1).min(53))
            );
            assert!(roi.area() <= 2601);
        }
    }
    p.at = (-1, 0);
    assert!(picker::region(app.editor.session.document(), &p).is_err());
    p.at = (53, 0);
    assert!(picker::region(app.editor.session.document(), &p).is_err());
}
#[test]
fn average_is_straight_encoded_rgba_half_up_including_hidden_rgb() {
    let roi = Rect::new(0, 0, 2, 1).unwrap();
    assert_eq!(
        picker::average(&[255, 0, 2, 0, 0, 1, 5, 255], roi).unwrap(),
        Rgba([128, 1, 4, 128])
    );
    assert!(picker::average(&[1; 7], roi).is_err());
    assert!(picker::average(&[0; 4], Rect::new(0, 0, 52, 51).unwrap()).is_err());
}
#[test]
fn publication_updates_only_requested_color_and_after_tool_without_history() {
    for secondary in [false, true] {
        for after in [None, Some(Tool::Pencil), Some(Tool::Brush)] {
            let (ctx, mut app) = setup();
            let source = app.editor.session.snapshot();
            let ticket = app.editor.session.history_ticket();
            let selection = app.editor.session.selection();
            let mut p = pick(&app);
            p.return_to = after;
            if secondary {
                p.button = PointerButton::Secondary;
                p.original = app.editor.tools.secondary;
            }
            assert!(app.queue_noise_picker(&Action::PickColor(p)));
            prepared_outcome(&mut app, Rgba([7, 11, 19, 0]));
            app.pump_flat_action(&ctx);
            assert_eq!(
                if secondary {
                    app.controls.secondary
                } else {
                    app.controls.primary
                },
                Rgba([7, 11, 19, 0])
            );
            assert_eq!(app.controls.tool, after.unwrap_or(Tool::ColorPicker));
            assert_eq!(app.editor.session.history_ticket(), ticket);
            assert_eq!(app.editor.session.undo_depth(), 0);
            assert_eq!(app.editor.session.selection(), selection);
            assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
        }
    }
}
#[test]
fn same_revision_identical_click_cancels_old_and_keeps_only_latest_wanted() {
    let (ctx, mut app) = setup();
    let p = pick(&app);
    app.queue_noise_picker(&Action::PickColor(p.clone()));
    prepared_outcome(&mut app, Rgba([1, 2, 3, 255]));
    let old_flag = app
        .filter_display
        .copy_action
        .as_ref()
        .unwrap()
        .cancelled
        .clone();
    app.queue_noise_picker(&Action::PickColor(p.clone()));
    assert!(old_flag.get());
    app.queue_noise_picker(&Action::PickColor(p));
    assert!(app.pump_flat_action(&ctx));
    assert_ne!(app.editor.tools.color, Rgba([1, 2, 3, 255]));
    assert!(app.filter_display.picker_wanted.is_some());
    assert!(app.filter_display.copy_action.is_none());
    prepared_outcome(&mut app, Rgba([4, 5, 6, 255]));
    app.pump_flat_action(&ctx);
    assert_eq!(app.editor.tools.color, Rgba([4, 5, 6, 255]));
}
#[test]
fn late_result_refuses_reselected_tool_color_options_owner_and_private_draft() {
    for change in 0..5 {
        let (ctx, mut app) = setup();
        app.queue_noise_picker(&Action::PickColor(pick(&app)));
        prepared_outcome(&mut app, Rgba([3, 5, 9, 255]));
        match change {
            0 => app.editor.tools.choose_tool(Tool::ColorPicker),
            1 => app.editor.tools.color = Rgba([21, 22, 23, 255]),
            2 => app.editor.tools.options.picker_size = SampleSize::Three,
            3 => app.editor = Editor::from_document(app.editor.session.document().clone()).unwrap(),
            _ => {
                let operation = state::operation(&app, Intent::Add(filters::Kind::Invert)).unwrap();
                let draft = Draft::capture(&app, operation).unwrap();
                app.filter_editing = Some(State {
                    draft,
                    base_texture: None,
                    base_footprint: None,
                    base_view: None,
                });
            }
        }
        let before = app.editor.tools.color;
        app.pump_flat_action(&ctx);
        assert_eq!(app.editor.tools.color, before);
        assert!(app.filter_display.copy_action.is_none());
    }
}
#[test]
fn picker_and_encoded_action_share_one_queue_and_escape_discards_wanted() {
    let (ctx, mut app) = setup();
    app.queue_noise_picker(&Action::PickColor(pick(&app)));
    let source = app.editor.session.snapshot();
    assert!(app.queue_noise_print(&ctx, source, "print".into()).is_err());
    app.cancel_background_image();
    assert!(app.filter_display.picker_wanted.is_none());
    assert!(app.filter_display.copy_action.is_none());
}
#[test]
fn layer_raw_and_old_image_routes_remain_synchronous() {
    let (_, mut app) = setup();
    let mut p = pick(&app);
    p.source = SampleSource::Layer;
    assert!(!app.queue_noise_picker(&Action::PickColor(p)));
    app.editor = Editor::new(2, 1).unwrap();
    app.editor.tools.choose_tool(Tool::ColorPicker);
    assert!(!app.queue_noise_picker(&Action::PickColor(pick(&app))));
}

#[test]
fn accepted_controls_change_before_canvas_sync_cannot_be_overwritten_by_ready_pick() {
    for change in 0..4 {
        let (ctx, mut app) = setup();
        app.queue_noise_picker(&Action::PickColor(pick(&app)));
        prepared_outcome(&mut app, Rgba([3, 5, 9, 255]));
        let old_tools_color = app.editor.tools.color;
        let old_tools_options = app.editor.tools.options;
        // Exact late-frame authoritative assignment used by Edit Colors and
        // toolbar widgets. Do not synchronize ToolState before the pump.
        match change {
            0 => app.controls.primary = Rgba([51, 52, 53, 255]),
            1 => app.controls.secondary = Rgba([61, 62, 63, 255]),
            2 => app.controls.tool = Tool::Pencil,
            _ => app.controls.options.picker_size = SampleSize::FiftyOne,
        }
        let colors = (app.controls.primary, app.controls.secondary);
        let tool = app.controls.tool;
        let options = app.controls.options;
        let ticket = app.editor.session.history_ticket();
        assert_eq!(app.editor.tools.color, old_tools_color);
        assert_eq!(app.editor.tools.options, old_tools_options);
        app.pump_flat_action(&ctx);
        assert_eq!((app.controls.primary, app.controls.secondary), colors);
        assert_eq!(app.controls.tool, tool);
        assert_eq!(app.controls.options, options);
        assert_eq!(app.editor.tools.color, old_tools_color);
        assert_eq!(app.editor.session.history_ticket(), ticket);
        assert!(app.filter_display.copy_action.is_none());
    }
}
#[test]
fn separately_forced_picker_tool_options_do_not_require_controls_equality() {
    let (ctx, mut app) = setup();
    app.controls.options.picker_size = SampleSize::FiftyOne;
    app.editor.tools.options.picker_size = SampleSize::One;
    app.queue_noise_picker(&Action::PickColor(pick(&app)));
    prepared_outcome(&mut app, Rgba([3, 5, 9, 255]));
    app.pump_flat_action(&ctx);
    assert_eq!(app.controls.primary, Rgba([3, 5, 9, 255]));
    assert_eq!(app.controls.options.picker_size, SampleSize::FiftyOne);
}
