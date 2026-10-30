//! Causal host delivery controls; actual Worker protocol is qualified separately.
use super::*;
use paintcraft_core::types::{Rect, Rgba, filters::Descriptor, resources::SharedBudget};
use paintcraft_document::{FilterInstance, FilterStack};
use std::sync::Arc;
fn render(ctx: &egui::Context, app: &mut WebApp) -> Vec<egui::Color32> {
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.test_refresh_texture(ui.ctx())
    });
    let id = app.canvas.texture.as_ref().unwrap().id();
    let pixels = output
        .textures_delta
        .set
        .iter()
        .find_map(|(texture, deltas)| {
            if *texture != id {
                return None;
            }
            let egui::ImageData::Color(image) = &deltas.last()?.image;
            Some(image.pixels.clone())
        })
        .unwrap_or_default();
    output.textures_delta.clear();
    pixels
}
fn setup() -> (egui::Context, WebApp, render::Reply) {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = Frontend::Studio;
    let mut doc = Editor::new(128, 80).unwrap().session.document().clone();
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
            Uuid::from_u128(1),
            true,
            Descriptor::InvertRgbV1,
        )])
        .unwrap();
    app.editor = Editor::from_document(doc).unwrap();
    let source = app.filter_capture();
    app.filter_display.source = Some(source.clone());
    let view = View::new(&source.document, Rect::new(0, 0, 128, 80).unwrap(), 0).unwrap();
    app.canvas.filter_view = Some(view);
    let reply = render::Reply {
        request: Request {
            revision: Revision {
                draft: source.draft,
                generation: 1,
            },
            edit: None,
            render: true,
            view,
        },
        size: (128, 80),
        pixels: Some(paintcraft_render::render_flat(&source.document).unwrap()),
    };
    (ctx, app, reply)
}
#[test]
fn late_committed_base_preserves_five_private_pixel_routes_and_cancel_restores_cache() {
    for mode in 0..5 {
        let (ctx, mut app, mut reply) = setup();
        if mode == 3 {
            app.editor
                .apply(Edit::Select {
                    corners: Some(((3, 3), (6, 6))),
                })
                .unwrap();
            let capture = app.filter_capture();
            reply.request.revision.draft = capture.draft;
            app.filter_display.source = Some(capture);
        }
        let source = app.editor.session.snapshot();
        let ticket = app.editor.session.ticket();
        let depth = app.editor.session.undo_depth();
        app.editor.tools.color = Rgba([255, 0, 0, 255]);
        if mode == 0 {
            app.editor.tools.tool = Tool::Pencil;
            app.editor.tools.options.size = 1;
            app.editor
                .tools
                .pointer_down(app.editor.target.unwrap(), (8, 8));
        } else if mode == 3 {
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
        } else if mode == 4 {
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
            app.text.style.points = 14;
            app.sync_text();
            app.editor
                .tools
                .begin_text(app.editor.target.unwrap(), (6, 6));
            app.editor.tools.type_text("A");
            if mode == 2 {
                app.test_start_preedit("B");
            }
        }
        let private = render(&ctx, &mut app);
        assert!(!private.is_empty());
        assert_ne!(
            private,
            egui::ColorImage::from_rgba_unmultiplied([128, 80], reply.pixels.as_ref().unwrap())
                .pixels
        );
        let id = app.canvas.texture.as_ref().unwrap().id();
        let stamp = app.editor.tools.text_stamp();
        let revision = app.editor.tools.preview_revision();
        let paste = app.paste_revision();
        app.accept_filter_reply(&ctx, reply);
        assert_eq!(
            app.canvas.texture.as_ref().unwrap().id(),
            id,
            "late base clobbered mode {mode}"
        );
        assert!(
            render(&ctx, &mut app).is_empty(),
            "current private key must not be replaced"
        );
        assert_eq!(app.editor.tools.text_stamp(), stamp);
        assert_eq!(app.editor.tools.preview_revision(), revision);
        assert_eq!(app.paste_revision(), paste);
        assert_eq!(app.editor.session.ticket(), ticket);
        assert_eq!(app.editor.session.undo_depth(), depth);
        assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
        app.text.reset_composition();
        app.editor.cancel();
        app.cancel_paste(&ctx);
        render(&ctx, &mut app);
        assert_eq!(
            app.canvas.texture.as_ref().unwrap().id(),
            app.filter_display.texture.as_ref().unwrap().id()
        );
        assert!(
            app.filter_display
                .committed
                .as_ref()
                .is_some_and(|s| app.filter_source_current(s))
        );
    }
}
#[test]
fn base_from_replaced_owner_is_never_cached_or_presented() {
    let (ctx, mut app, reply) = setup();
    let original = app.editor.session.snapshot();
    app.editor = Editor::from_document((*original).clone()).unwrap();
    app.accept_filter_reply(&ctx, reply);
    assert!(app.canvas.texture.is_none());
    assert!(app.filter_display.texture.is_none());
    assert_eq!(app.editor.session.undo_depth(), 0);
}

#[test]
fn cancelling_private_pixels_in_a_disjoint_roi_never_keeps_the_old_private_texture() {
    let (ctx, mut app, reply) = setup();
    app.editor.tools.tool = Tool::Pencil;
    app.editor.tools.color = Rgba([255, 0, 0, 255]);
    app.editor
        .tools
        .pointer_down(app.editor.target.unwrap(), (8, 8));
    render(&ctx, &mut app);
    let private = app.canvas.texture.as_ref().unwrap().id();
    app.accept_filter_reply(&ctx, reply);
    let base = app.filter_display.texture.as_ref().unwrap().id();
    assert_ne!(private, base);
    app.canvas.filter_view = Some(
        View::new(
            app.editor.session.document(),
            Rect::new(100, 60, 20, 20).unwrap(),
            0,
        )
        .unwrap(),
    );
    app.editor.cancel();
    render(&ctx, &mut app);
    assert_eq!(
        app.canvas.texture.as_ref().unwrap().id(),
        base,
        "oldprivatepixelscannotstandinforanewpendingROI"
    );
    assert_eq!(
        app.canvas.filter_footprint,
        Some(Rect::new(0, 0, 128, 80).unwrap())
    );
}
#[test]
fn immediate_filter_open_after_ordinary_cancel_never_captures_an_old_private_image() {
    for cached in [false, true] {
        let (ctx, mut app, reply) = setup();
        app.editor.tools.tool = Tool::Pencil;
        app.editor.tools.color = Rgba([255, 0, 0, 255]);
        app.editor
            .tools
            .pointer_down(app.editor.target.unwrap(), (8, 8));
        render(&ctx, &mut app);
        let private = app.canvas.texture.as_ref().unwrap().id();
        if cached {
            app.accept_filter_reply(&ctx, reply);
        }
        app.editor.cancel();
        app.canvas.filter_view = Some(
            View::new(
                app.editor.session.document(),
                Rect::new(100, 60, 20, 20).unwrap(),
                0,
            )
            .unwrap(),
        );
        app.begin_studio_filter(&ctx, Intent::Add(filters::Kind::Invert));
        let state = app.filter_editing.as_ref().unwrap();
        assert_ne!(
            state.base_texture.as_ref().map(egui::TextureHandle::id),
            Some(private)
        );
        if cached {
            assert_eq!(
                state.base_texture.as_ref().unwrap().id(),
                app.filter_display.texture.as_ref().unwrap().id()
            );
        } else {
            assert!(state.base_texture.is_none());
            assert!(state.base_view.is_none());
        }
    }
}
