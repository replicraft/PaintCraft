//! Headless capture/consent/handoff tests, not actual Worker delivery claims.
use super::*;
use paintcraft_core::types::filters::{AddNoiseRgbV1, Descriptor};
use paintcraft_document::{FilterInstance, FilterStack};
use std::sync::Arc;
fn setup() -> (egui::Context, WebApp) {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
    let mut doc = Editor::new(2, 1).unwrap().session.document().clone();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(88),
            false,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
        )])
        .unwrap();
    app.editor = Editor::from_document(doc).unwrap();
    (ctx, app)
}
fn options(doc: &paintcraft_document::Document, bake: bool) -> crate::files::ExportOptions {
    use paintcraft_formats::{
        export::{filter_bake, metadata_policy},
        registry::{Family, Operation},
    };
    let target = metadata_policy::Target::new(Family::Png, Operation::ExportFlat);
    crate::files::ExportOptions {
        format: Some(ImageExport::Png),
        jpeg_quality: 90,
        jpeg_matte: None,
        pcx_encoding: paintcraft_ui::pcx_export::Encoding::Rgb24,
        frontend: paintcraft_formats::registry::Frontend::Studio,
        source_metadata: metadata_policy::Intent::Refuse,
        filter_bake: filter_bake::Policy::capture(
            doc,
            target,
            if bake {
                filter_bake::Intent::Bake
            } else {
                filter_bake::Intent::Refuse
            },
            &|| Ok(()),
        )
        .unwrap(),
    }
}
#[test]
fn actual_frozen_export_requires_bake_and_does_not_publish_or_enter_sync_renderer() {
    let (ctx, mut app) = setup();
    let source = app.editor.session.snapshot();
    let before = app.editor.session.ticket();
    assert!(
        app.queue_noise_download(
            &ctx,
            source.clone(),
            options(&source, false),
            "copy.png".into()
        )
        .is_err()
    );
    assert!(app.filter_display.copy_action.is_none());
    app.queue_noise_download(
        &ctx,
        source.clone(),
        options(&source, true),
        "copy.png".into(),
    )
    .unwrap();
    assert!(
        app.queue_noise_print(&ctx, source.clone(), "print".into())
            .is_err()
    );
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.ticket(), before);
    assert_eq!(app.editor.session.undo_depth(), 0);
    app.cancel_background_image();
    app.pump_flat_action(&ctx);
    assert!(app.filter_display.copy_action.is_none());
    assert_eq!(app.editor.session.ticket(), before);
}
#[test]
fn source_replacement_cancels_pending_action_before_acquisition_and_preserves_last_texture() {
    let (ctx, mut app) = setup();
    let source = app.editor.session.snapshot();
    app.queue_noise_print(&ctx, source.clone(), "print".into())
        .unwrap();
    app.editor = Editor::from_document((*source).clone()).unwrap();
    app.pump_flat_action(&ctx);
    assert!(app.filter_display.copy_action.is_none());
    assert_eq!(app.editor.session.undo_depth(), 0);
}
#[test]
fn unsupported_encoder_is_explicit_and_leaves_source_pixels_unchanged() {
    let (ctx, mut app) = setup();
    let source = app.editor.session.snapshot();
    let mut options = options(&source, true);
    options.format = Some(ImageExport::Dds);
    assert!(
        app.queue_noise_download(&ctx, source.clone(), options, "copy.dds".into())
            .is_err()
    );
    assert!(app.filter_display.copy_action.is_none());
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
}

#[test]
fn new_private_filter_draft_invalidates_frozen_background_intent() {
    let (ctx, mut app) = setup();
    let source = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    let revision = app.editor.tools.preview_revision();
    app.queue_noise_print(&ctx, source, "print".into()).unwrap();
    let operation = state::operation(&app, Intent::Add(filters::Kind::Invert)).unwrap();
    let draft = Draft::capture(&app, operation).unwrap();
    app.filter_editing = Some(State {
        draft,
        base_texture: None,
        base_footprint: None,
        base_view: None,
    });
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.tools.preview_revision(), revision);
    app.pump_flat_action(&ctx);
    assert!(app.filter_display.copy_action.is_none());
    assert!(app.filter_editing.is_some());
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.session.undo_depth(), 0);
}
#[test]
fn private_paste_start_invalidates_pending_background_intent() {
    let (ctx, mut app) = setup();
    app.controls.tool = Tool::Select;
    app.editor.tools.activate_tool(Tool::Select);
    let revision = app.editor.tools.preview_revision();
    let source = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    app.queue_noise_print(&ctx, source, "print".into()).unwrap();
    let image = paintcraft_core::types::clipboard::Image::from_rgba(
        1,
        1,
        vec![255, 0, 0, 255],
        &paintcraft_core::types::resources::SharedBudget::new(16),
        &|| Ok(()),
    )
    .unwrap();
    app.test_place_image(&ctx, image);
    assert!(app.paste_revision().is_some());
    assert_eq!(
        app.editor.tools.preview_revision(),
        revision,
        "paste fence must be independent of tool revision"
    );
    app.pump_flat_action(&ctx);
    assert!(app.filter_display.copy_action.is_none());
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.session.undo_depth(), 0);
}

#[test]
fn ended_stroke_texture_is_restored_to_valid_base_before_action_handoff() {
    let (ctx, mut app) = setup();
    let source = app.filter_capture();
    let view = View::new(
        &source.document,
        paintcraft_core::types::Rect::new(0, 0, 2, 1).unwrap(),
        0,
    )
    .unwrap();
    app.canvas.filter_view = Some(view);
    app.filter_display.source = Some(source.clone());
    let reply = |capture: &Capture, pixels: Vec<u8>| render::Reply {
        request: Request {
            revision: Revision {
                draft: capture.draft,
                generation: 1,
            },
            edit: None,
            render: true,
            view,
        },
        size: (2, 1),
        pixels: Some(pixels),
    };
    // Stored Noise is disabled in this fixture, so these two literal white source
    // pixels and a one-pixel Pencil preview require no expected filter evaluator.
    app.accept_filter_reply(&ctx, reply(&source, vec![255; 8]));
    let base = app.canvas.texture.as_ref().unwrap().id();
    app.editor.tools.tool = Tool::Pencil;
    app.editor.tools.options.size = 1;
    app.editor.tools.color = paintcraft_core::types::Rgba([255, 0, 0, 255]);
    app.editor
        .tools
        .pointer_down(app.editor.target.unwrap(), (0, 0));
    app.worker_filter_canvas(&ctx);
    let transient = app
        .filter_display
        .transient
        .as_ref()
        .unwrap()
        .capture
        .clone()
        .unwrap();
    app.accept_filter_reply(
        &ctx,
        reply(&transient, vec![255, 0, 0, 255, 255, 255, 255, 255]),
    );
    assert_ne!(app.canvas.texture.as_ref().unwrap().id(), base);
    app.editor.cancel();
    let ticket = app.editor.session.ticket();
    app.queue_noise_print(&ctx, app.editor.session.snapshot(), "print".into())
        .unwrap();
    assert!(app.filter_display.transient.is_none());
    assert_eq!(app.canvas.texture.as_ref().unwrap().id(), base);
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.session.undo_depth(), 0);
    app.cancel_background_image();
    app.pump_flat_action(&ctx);
    assert!(app.filter_display.copy_action.is_none());
    assert_eq!(app.canvas.texture.as_ref().unwrap().id(), base);
}
