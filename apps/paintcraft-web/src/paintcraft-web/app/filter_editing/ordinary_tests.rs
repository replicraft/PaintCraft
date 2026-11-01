//! Headless host lifecycle tests; actual Worker/Chrome qualification is separate.
use super::*;
use paintcraft_core::types::{
    Rect, Rgba,
    filters::{AddNoiseRgbV1, Descriptor},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{Document, FilterInstance, FilterStack};
use paintcraft_render::noise_work::{NoiseAdmission, NoiseJobPolicy, with_noise_workspace};
use std::sync::Arc;
fn pixels(doc: &Document) -> Vec<u8> {
    let budget = SharedBudget::new(64 << 20);
    let root = NoiseAdmission::new_root(
        budget.scope(AllocationClass::SharedBudget),
        NoiseJobPolicy::BrowserV1,
        NoiseJobPolicy::BrowserV1.max_limits(),
    )
    .unwrap();
    let result = with_noise_workspace(root, || paintcraft_render::render_flat(doc)).unwrap();
    assert_eq!(budget.usage().reserved, 0);
    result
}
fn reply(capture: &Capture, view: View) -> render::Reply {
    render::Reply {
        request: Request {
            revision: Revision {
                draft: capture.draft,
                generation: 1,
            },
            edit: None,
            render: true,
            view,
        },
        size: view.size.into(),
        pixels: Some(pixels(&capture.document)),
    }
}
fn setup() -> (egui::Context, WebApp) {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = Frontend::Studio;
    let mut doc = Editor::new(64, 40).unwrap().session.document().clone();
    doc.layers
        .get_mut(&doc.roots[0])
        .unwrap()
        .raster
        .blend_pixel(
            4,
            4,
            Rgba([255, 0, 0, 255]),
            paintcraft_core::types::policy::BlendMode::Replace,
            &mut paintcraft_raster::Admission::new(262144),
        )
        .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(901),
            true,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
        )])
        .unwrap();
    app.editor = Editor::from_document(doc).unwrap();
    let source = app.filter_capture();
    let view = View::new(&source.document, Rect::new(0, 0, 64, 40).unwrap(), 0).unwrap();
    app.canvas.filter_view = Some(view);
    app.filter_display.source = Some(source.clone());
    app.accept_filter_reply(&ctx, reply(&source, view));
    (ctx, app)
}
fn pending(app: &WebApp) -> Capture {
    app.filter_display
        .transient
        .as_ref()
        .unwrap()
        .capture
        .clone()
        .unwrap()
}
fn brush(app: &mut WebApp) {
    app.editor.tools.tool = Tool::Pencil;
    app.editor.tools.options.size = 1;
    app.editor.tools.color = Rgba([255, 0, 0, 255]);
    app.editor
        .tools
        .pointer_down(app.editor.target.unwrap(), (8, 8));
}
#[test]
fn noise_stroke_preview_reuses_ephemeral_source_and_cancel_restores_base_without_publication() {
    let (ctx, mut app) = setup();
    let source = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    let base = app.canvas.texture.as_ref().unwrap().id();
    brush(&mut app);
    assert!(app.worker_filter_canvas(&ctx));
    let capture = pending(&app);
    assert!(capture.scoped);
    assert_eq!(capture.ticket.state_id, capture.document.state_id);
    assert_ne!(
        capture.document.layers[&source.roots[0]].raster,
        source.layers[&source.roots[0]].raster
    );
    app.worker_filter_canvas(&ctx);
    assert!(Arc::ptr_eq(&capture.document, &pending(&app).document));
    let frame = reply(&capture, app.canvas.filter_view.unwrap());
    app.accept_filter_reply(&ctx, frame);
    assert_ne!(app.canvas.texture.as_ref().unwrap().id(), base);
    assert!(Arc::ptr_eq(
        &app.filter_display.committed.as_ref().unwrap().document,
        &source
    ));
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.session.undo_depth(), 0);
    assert!(app.editor.tools.gesture_in_progress());
    app.editor.cancel();
    app.worker_filter_canvas(&ctx);
    assert!(app.filter_display.transient.is_none());
    assert_eq!(app.canvas.texture.as_ref().unwrap().id(), base);
    app.accept_filter_reply(&ctx, reply(&capture, app.canvas.filter_view.unwrap()));
    assert_eq!(app.canvas.texture.as_ref().unwrap().id(), base);
}
#[test]
fn changed_stroke_and_view_discard_late_source_six_frames() {
    let (ctx, mut app) = setup();
    brush(&mut app);
    app.worker_filter_canvas(&ctx);
    let first = pending(&app);
    let old_view = app.canvas.filter_view.unwrap();
    app.editor.tools.pointer_move((10, 8));
    app.worker_filter_canvas(&ctx);
    let second = pending(&app);
    assert!(!Arc::ptr_eq(&first.document, &second.document));
    assert_ne!(first.draft, second.draft);
    let base = app.canvas.texture.as_ref().unwrap().id();
    app.accept_filter_reply(&ctx, reply(&first, old_view));
    assert_eq!(app.canvas.texture.as_ref().unwrap().id(), base);
    app.canvas.filter_view =
        Some(View::new(&second.document, Rect::new(0, 0, 32, 20).unwrap(), 0).unwrap());
    app.accept_filter_reply(&ctx, reply(&second, old_view));
    assert_eq!(app.canvas.texture.as_ref().unwrap().id(), base);
}
#[test]
fn noise_text_preview_commit_requires_a_fresh_committed_base_and_ignores_late_transient() {
    let (ctx, mut app) = setup();
    app.controls.tool = Tool::Text;
    app.editor.tools.choose_tool(Tool::Text);
    app.text.style.points = 14;
    app.sync_text();
    app.editor
        .tools
        .begin_text(app.editor.target.unwrap(), (6, 6));
    app.editor.tools.type_text("A");
    let before = app.editor.session.ticket();
    app.worker_filter_canvas(&ctx);
    let capture = pending(&app);
    app.accept_filter_reply(&ctx, reply(&capture, app.canvas.filter_view.unwrap()));
    assert_eq!(app.editor.session.ticket(), before);
    assert!(app.editor.tools.is_editing_text());
    app.editor.place_text().unwrap();
    assert_ne!(app.editor.session.ticket(), before);
    app.worker_filter_canvas(&ctx);
    assert!(app.filter_display.transient.is_none());
    let committed = app.filter_display.source.clone().unwrap();
    assert!(app.filter_source_current(&committed));
    app.accept_filter_reply(&ctx, reply(&committed, app.canvas.filter_view.unwrap()));
    let base = app.canvas.texture.as_ref().unwrap().id();
    app.accept_filter_reply(&ctx, reply(&capture, app.canvas.filter_view.unwrap()));
    assert_eq!(app.canvas.texture.as_ref().unwrap().id(), base);
    assert!(app.filter_source_current(app.filter_display.committed.as_ref().unwrap()));
    assert_eq!(app.editor.session.undo_depth(), 1);
}
#[test]
fn noise_ime_and_paste_ephemeral_sources_cancel_without_becoming_committed() {
    for paste in [false, true] {
        let (ctx, mut app) = setup();
        let ticket = app.editor.session.ticket();
        let base = app.canvas.texture.as_ref().unwrap().id();
        if paste {
            let image = paintcraft_core::types::clipboard::Image::from_rgba(
                1,
                1,
                vec![255, 0, 0, 255],
                &SharedBudget::new(16),
                &|| Ok(()),
            )
            .unwrap();
            app.test_place_image(&ctx, image);
        } else {
            app.controls.tool = Tool::Text;
            app.editor.tools.choose_tool(Tool::Text);
            app.sync_text();
            app.editor
                .tools
                .begin_text(app.editor.target.unwrap(), (6, 6));
            app.editor.tools.type_text("A");
            app.test_start_preedit("B");
        }
        app.worker_filter_canvas(&ctx);
        let capture = pending(&app);
        assert_ne!(
            capture.document.layers[&capture.document.roots[0]].raster,
            app.editor.session.document().layers[&app.editor.target.unwrap()].raster
        );
        app.accept_filter_reply(&ctx, reply(&capture, app.canvas.filter_view.unwrap()));
        assert_eq!(app.editor.session.ticket(), ticket);
        app.text.reset_composition();
        app.editor.cancel();
        app.cancel_paste(&ctx);
        app.worker_filter_canvas(&ctx);
        assert_eq!(app.canvas.texture.as_ref().unwrap().id(), base);
        assert_eq!(app.editor.session.ticket(), ticket);
    }
}
#[test]
fn stroke_commit_and_text_cancel_preserve_publication_and_base_boundaries() {
    for text in [false, true] {
        let (ctx, mut app) = setup();
        let before = app.editor.session.ticket();
        let base = app.canvas.texture.as_ref().unwrap().id();
        if text {
            app.controls.tool = Tool::Text;
            app.editor.tools.choose_tool(Tool::Text);
            app.sync_text();
            app.editor
                .tools
                .begin_text(app.editor.target.unwrap(), (6, 6));
            app.editor.tools.type_text("A");
        } else {
            brush(&mut app);
        }
        app.worker_filter_canvas(&ctx);
        let capture = pending(&app);
        app.accept_filter_reply(&ctx, reply(&capture, app.canvas.filter_view.unwrap()));
        if text {
            app.editor.cancel();
            app.worker_filter_canvas(&ctx);
            assert_eq!(app.editor.session.ticket(), before);
            assert_eq!(app.editor.session.undo_depth(), 0);
            assert_eq!(app.canvas.texture.as_ref().unwrap().id(), base);
        } else {
            let (_, action) = app.editor.tools.pointer_up().unwrap();
            app.editor.apply(action).unwrap();
            app.worker_filter_canvas(&ctx);
            assert_ne!(app.editor.session.ticket(), before);
            assert_eq!(app.editor.session.undo_depth(), 1);
            let committed = app.filter_display.source.clone().unwrap();
            app.accept_filter_reply(&ctx, reply(&committed, app.canvas.filter_view.unwrap()));
            assert!(app.filter_source_current(app.filter_display.committed.as_ref().unwrap()));
        }
        let current = app.canvas.texture.as_ref().unwrap().id();
        app.accept_filter_reply(&ctx, reply(&capture, app.canvas.filter_view.unwrap()));
        assert_eq!(app.canvas.texture.as_ref().unwrap().id(), current);
    }
}
#[test]
fn owner_replacement_and_staging_refusal_never_publish_or_cancel_text() {
    let (ctx, mut app) = setup();
    brush(&mut app);
    app.worker_filter_canvas(&ctx);
    let capture = pending(&app);
    let source = app.editor.session.snapshot();
    app.editor = Editor::from_document((*source).clone()).unwrap();
    let current = app.canvas.texture.as_ref().unwrap().id();
    app.accept_filter_reply(&ctx, reply(&capture, app.canvas.filter_view.unwrap()));
    assert_eq!(app.canvas.texture.as_ref().unwrap().id(), current);
    app.controls.tool = Tool::Text;
    app.editor.tools.choose_tool(Tool::Text);
    app.sync_text();
    app.editor
        .tools
        .begin_text(app.editor.target.unwrap(), (6, 6));
    app.editor.tools.type_text("A");
    let mut invalid = app.editor.tools.text_style.clone().unwrap();
    invalid.points = 0;
    app.editor.tools.set_text_style(invalid);
    let before = app.editor.session.ticket();
    app.worker_filter_canvas(&ctx);
    assert!(
        app.filter_display
            .transient
            .as_ref()
            .unwrap()
            .capture
            .is_none()
    );
    assert!(app.editor.tools.is_editing_text());
    assert_eq!(app.editor.tools.text_content(), Some("A"));
    assert_eq!(app.editor.session.ticket(), before);
    assert_eq!(app.editor.session.undo_depth(), 0);
}

#[test]
fn noise_floating_selection_refuses_coordinate_remap_and_cancel_preserves_base() {
    let (ctx, mut app) = setup();
    app.editor
        .apply(Edit::Select {
            corners: Some(((3, 3), (6, 6))),
        })
        .unwrap();
    let base_source = app.filter_capture();
    app.filter_display.source = Some(base_source.clone());
    app.accept_filter_reply(&ctx, reply(&base_source, app.canvas.filter_view.unwrap()));
    let base = app.canvas.texture.as_ref().unwrap().id();
    let ticket = app.editor.session.ticket();
    app.editor.tools.tool = Tool::Select;
    assert!(
        app.editor
            .begin_floating(
                Rect::new(3, 3, 4, 4).unwrap(),
                (4, 4),
                paintcraft_input::PointerButton::Primary
            )
            .unwrap()
    );
    app.editor.tools.pointer_move((9, 4));
    app.worker_filter_canvas(&ctx);
    // Source-coordinate remapping currently requires an explicit bake. A failed
    // private staging attempt must preserve the gesture and last good display.
    let failed = app.filter_display.transient.as_ref().unwrap();
    assert!(failed.capture.is_none());
    assert!(app.message.contains("require an explicit bake"));
    let message = app.message.clone();
    app.message.clear();
    assert!(app.worker_filter_canvas(&ctx));
    assert!(
        app.message.is_empty(),
        "same failed revision must not restage"
    );
    assert!(!message.is_empty());
    assert_eq!(app.canvas.texture.as_ref().unwrap().id(), base);
    assert!(Arc::ptr_eq(
        &app.editor.session.snapshot(),
        &base_source.document
    ));
    assert_eq!(app.editor.session.ticket(), ticket);
    assert!(app.editor.tools.floating_selection().is_some());
    app.editor.cancel();
    app.worker_filter_canvas(&ctx);
    assert_eq!(app.canvas.texture.as_ref().unwrap().id(), base);
    assert!(app.filter_source_current(app.filter_display.committed.as_ref().unwrap()));
}
