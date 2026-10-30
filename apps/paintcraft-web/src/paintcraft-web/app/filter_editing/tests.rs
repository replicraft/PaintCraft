//! Host lifecycle controls. Real Worker execution is qualified separately.
use super::*;
use paintcraft_core::types::{Rect, Rgba, filters::Descriptor};
use paintcraft_studio::filters::Kind;
use paintcraft_ui::actions::ViewAction;
use std::sync::Arc;
fn setup() -> (egui::Context, WebApp) {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = Frontend::Studio;
    app.editor = Editor::new(8, 4).unwrap();
    app.canvas.filter_view = Some(
        View::new(
            app.editor.session.document(),
            Rect::new(0, 0, 8, 4).unwrap(),
            0,
        )
        .unwrap(),
    );
    (ctx, app)
}
fn candidate(app: &WebApp) -> Request {
    let draft = &app.filter_editing.as_ref().unwrap().draft;
    Request {
        revision: draft.revision,
        edit: Some(draft.operation().unwrap()),
        render: draft.parameters.preview,
        view: app.canvas.filter_view.unwrap(),
    }
}
#[test]
fn parameters_are_private_apply_requires_exact_generation_and_commits_once() {
    let (ctx, mut app) = setup();
    let source = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    app.begin_studio_filter(&ctx, Intent::Add(Kind::BrightnessContrast));
    let old = candidate(&app);
    app.filter_editing
        .as_mut()
        .unwrap()
        .draft
        .parameters
        .brightness = 30;
    app.filter_editing.as_mut().unwrap().draft.change().unwrap();
    let request = candidate(&app);
    let preview = app
        .editor
        .display_preview(Some(paintcraft_tools::Preview::Filter(
            request.edit.clone().unwrap(),
        )))
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.ticket(), ticket);
    app.filter_display.renderer.accept_test(old);
    let draft = &mut app.filter_editing.as_mut().unwrap().draft;
    draft.apply = Some(draft.revision);
    app.finish_filter_apply(&ctx);
    assert!(app.filter_editing.is_some());
    assert_eq!(app.editor.session.undo_depth(), 0);
    app.filter_display.renderer.accept_test(request);
    app.finish_filter_apply(&ctx);
    assert!(app.filter_editing.is_none());
    assert_eq!(app.editor.session.undo_depth(), 1);
    assert_eq!(
        paintcraft_render::render_flat(app.editor.session.document()).unwrap(),
        paintcraft_render::render_flat(&preview).unwrap()
    );
    app.finish_filter_apply(&ctx);
    assert_eq!(app.editor.session.undo_depth(), 1);
}
#[test]
fn preview_off_requires_current_candidate_admission_but_not_base_raster() {
    let (ctx, mut app) = setup();
    app.begin_studio_filter(&ctx, Intent::Add(Kind::Invert));
    let draft = &mut app.filter_editing.as_mut().unwrap().draft;
    draft.parameters.preview = false;
    draft.change().unwrap();
    draft.apply = Some(draft.revision);
    app.finish_filter_apply(&ctx);
    assert_eq!(app.editor.session.undo_depth(), 0);
    let draft = &mut app.filter_editing.as_mut().unwrap().draft;
    draft.prepared = Some(draft.revision);
    app.finish_filter_apply(&ctx);
    assert_eq!(app.editor.session.undo_depth(), 1);
    assert!(app.canvas.texture.is_none());
    assert_eq!(
        paintcraft_render::render_flat(app.editor.session.document()).unwrap(),
        [0, 0, 0, 255].repeat(32)
    );
}
#[test]
fn cancel_and_source_or_layer_change_retire_private_intent_without_overwriting_document() {
    for change in 0..4 {
        let (ctx, mut app) = setup();
        app.begin_studio_filter(&ctx, Intent::Add(Kind::Invert));
        let source = app.editor.session.snapshot();
        match change {
            0 => app.cancel_filter_edit(),
            1 => {
                app.editor = Editor::from_document((*source).clone()).unwrap();
                app.pump_filter_editing(&ctx);
            }
            2 => {
                app.editor.target = None;
                app.pump_filter_editing(&ctx);
            }
            _ => {
                app.frontend = Frontend::Classic;
                app.pump_filter_editing(&ctx);
            }
        }
        assert!(app.filter_editing.is_none());
        assert_eq!(app.editor.session.undo_depth(), 0);
        assert!(app.editor.session.document().same_semantics(&source));
    }
}
#[test]
fn pending_tool_drafts_and_mutating_actions_are_fenced_but_zoom_remains_nonmodal() {
    let (ctx, mut app) = setup();
    app.editor
        .tools
        .begin_text(app.editor.target.unwrap(), (0, 0));
    app.editor.tools.type_text("editable");
    let stamp = app.editor.tools.text_stamp();
    app.begin_studio_filter(&ctx, Intent::Add(Kind::Invert));
    assert!(app.filter_editing.is_none());
    assert_eq!(app.editor.tools.text_stamp(), stamp);
    app.editor.tools.cancel_text();
    app.begin_studio_filter(&ctx, Intent::Add(Kind::Invert));
    assert!(app.blocked());
    assert!(!app.modal_blocked());
    assert!(app.filter_navigation_ready());
    let source = app.editor.session.snapshot();
    let zoom = app.zoom;
    app.action(&ctx, Action::View(ViewAction::ZoomIn));
    assert!(app.zoom > zoom);
    app.action(&ctx, Action::ClearCanvas);
    app.edit(Edit::ClearCanvas {
        color: Rgba([9, 8, 7, 255]),
    });
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.undo_depth(), 0);
}
#[test]
fn real_engine_failure_retains_parameters_and_apply_can_retry() {
    fn refuse(_: &paintcraft_document::Document, _: &dyn Fn() -> Result<()>) -> Result<()> {
        Err(Error::new(
            ErrorCode::ResourceLimit,
            "test persistent admission",
        ))
    }
    let (ctx, mut app) = setup();
    app.begin_studio_filter(&ctx, Intent::Add(Kind::Invert));
    app.editor.session.set_persistent_validator(Some(refuse));
    let request = candidate(&app);
    app.filter_display.renderer.accept_test(request);
    let draft = &mut app.filter_editing.as_mut().unwrap().draft;
    draft.apply = Some(draft.revision);
    let source = app.editor.session.snapshot();
    app.finish_filter_apply(&ctx);
    let state = app.filter_editing.as_ref().unwrap();
    assert_eq!(
        state.draft.error.as_ref().unwrap().code,
        ErrorCode::ResourceLimit
    );
    assert!(state.draft.apply.is_none());
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    app.editor.session.set_persistent_validator(None);
    let draft = &mut app.filter_editing.as_mut().unwrap().draft;
    draft.apply = Some(draft.revision);
    app.finish_filter_apply(&ctx);
    assert_eq!(app.editor.session.undo_depth(), 1);
}
#[test]
fn direct_stack_rows_use_captured_dispatch_and_history_without_parameter_dialog() {
    let (ctx, mut app) = setup();
    app.begin_studio_filter(&ctx, Intent::Add(Kind::Invert));
    let request = candidate(&app);
    app.filter_display.renderer.accept_test(request);
    let draft = &mut app.filter_editing.as_mut().unwrap().draft;
    draft.apply = Some(draft.revision);
    app.finish_filter_apply(&ctx);
    let id = app.editor.session.document().layers[&app.editor.target.unwrap()]
        .filters
        .as_slice()[0]
        .id();
    for intent in [
        Intent::SetEnabled { id, enabled: false },
        Intent::Duplicate(id),
        Intent::Reorder { id, to_index: 1 },
        Intent::Remove(id),
    ] {
        app.begin_studio_filter(&ctx, intent);
        assert!(app.filter_editing.is_none());
    }
    assert_eq!(app.editor.session.undo_depth(), 5);
    let filters = &app.editor.session.document().layers[&app.editor.target.unwrap()].filters;
    assert_eq!(filters.len(), 1);
    assert_eq!(filters.as_slice()[0].descriptor(), Descriptor::InvertRgbV1);
    assert!(!filters.as_slice()[0].enabled());
}
#[test]
fn aligned_roi_and_display_level_do_not_expand_to_a_large_sparse_canvas() {
    let doc = paintcraft_document::new_document(
        100_000,
        80_000,
        Some(Rgba([17, 31, 47, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let view = View::new(&doc, Rect::new(101, 203, 13, 9).unwrap(), 2).unwrap();
    assert_eq!(view.region, Rect::new(100, 200, 16, 12).unwrap());
    assert_eq!(view.size, [4, 3]);
    assert_eq!(view.footprint, view.region);
    assert!(u64::from(view.size[0]) * u64::from(view.size[1]) < crate::editor::MAX_PIXELS);
    let edge = View::new(&doc, Rect::new(99_997, 79_998, 3, 2).unwrap(), 3).unwrap();
    assert_eq!(edge.region.x1, 100_000);
    assert_eq!(edge.region.y1, 80_000);
}

#[test]
fn cancelling_an_unfiltered_roi_base_restores_a_literal_full_texture_for_future_pans() {
    let (ctx, mut app) = setup();
    let mut first = ctx.run_ui(egui::RawInput::default(), |ui| app.show(ui));
    first.textures_delta.clear();
    app.begin_studio_filter(&ctx, Intent::Add(Kind::Invert));
    let roi = View::new(
        app.editor.session.document(),
        Rect::new(0, 0, 2, 1).unwrap(),
        0,
    )
    .unwrap();
    // Fixture for an acknowledged base FRAME while Preview is unchecked.
    app.install_filter_pixels(&ctx, (2, 1), roi.footprint, &[255; 8]);
    let state = app.filter_editing.as_mut().unwrap();
    state.draft.parameters.preview = false;
    state.base_texture = app.canvas.texture.clone();
    state.base_footprint = Some(roi.footprint);
    state.base_view = Some(roi);
    app.cancel_filter_edit();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show(ui));
    let id = app.canvas.texture.as_ref().unwrap().id();
    let full = output
        .textures_delta
        .set
        .iter()
        .find_map(|(texture, deltas)| {
            if *texture != id {
                return None;
            }
            let egui::ImageData::Color(image) = &deltas.last()?.image;
            Some(image.clone())
        })
        .expect("leaving Worker ROI mode must refresh the legacy full-canvas key");
    assert_eq!(full.size, [8, 4]);
    assert!(
        full.pixels
            .iter()
            .all(|pixel| *pixel == egui::Color32::WHITE)
    );
    assert!(app.canvas.filter_footprint.is_none());
    output.textures_delta.clear();
    app.canvas.filter_view = Some(
        View::new(
            app.editor.session.document(),
            Rect::new(6, 3, 2, 1).unwrap(),
            0,
        )
        .unwrap(),
    );
    assert_eq!(app.canvas.texture.as_ref().unwrap().size(), [8, 4]);
    assert_eq!(app.editor.session.undo_depth(), 0);
}
