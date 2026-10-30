use super::*;
use crate::editor::Editor;
use paintcraft_core::types::{Rgba, filters::Descriptor};
use paintcraft_document::{FilterInstance, FilterStack};
fn render(ctx: &egui::Context, app: &mut WebApp) -> Option<Vec<Color32>> {
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.refresh_texture(ui.ctx())
    });
    let id = app.canvas.texture.as_ref().unwrap().id();
    let image = output
        .textures_delta
        .set
        .iter()
        .find_map(|(texture, delta)| {
            if *texture != id {
                return None;
            }
            let egui::ImageData::Color(image) = &delta.last()?.image;
            Some(image.pixels.clone())
        });
    output.textures_delta.clear();
    image
}

#[test]
#[allow(deprecated)] // Exercise explicit compatibility cancellation.
fn composition_revision_refreshes_actual_texture_and_cancel_restores_base_without_publication() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let mut doc = Editor::new(128, 80).unwrap().session.document().clone();
    let layer = doc.roots[0];
    doc.layers.get_mut(&layer).unwrap().opacity = 150;
    doc.layers.get_mut(&layer).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        uuid::Uuid::from_u128(19),
        true,
        Descriptor::InvertRgbV1,
    )])
    .unwrap();
    app.editor = Editor::from_document(doc).unwrap();
    app.controls.tool = Tool::Text;
    app.editor.tools.choose_tool(Tool::Text);
    app.controls.primary = Rgba([60, 120, 190, 160]);
    app.sync_text();
    app.editor.tools.begin_text(layer, (6, 6));
    app.editor.tools.type_text("Base");
    let ticket = app.editor.session.ticket();
    let stamp = app.editor.tools.text_stamp();
    let base = render(&ctx, &mut app).unwrap();
    let key = app.canvas.key;
    app.text_event(
        &ctx,
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "日本".into(),
            active_range_chars: None,
        }),
    );
    let image =
        render(&ctx, &mut app).expect("preedit must invalidate texture without changing ToolState");
    assert_ne!(app.canvas.key, key);
    assert_ne!(image, base);
    assert_eq!(app.editor.tools.text_stamp(), stamp);
    let preview = app.canvas_preview_document().unwrap().unwrap();
    let bytes = paintcraft_render::render_flat(&preview).unwrap();
    assert_eq!(
        image,
        egui::ColorImage::from_rgba_unmultiplied([128, 80], &bytes).pixels
    );
    assert!(
        render(&ctx, &mut app).is_none(),
        "unchanged composition reuses exact derived texture"
    );
    app.text_event(&ctx, egui::Event::Ime(egui::ImeEvent::Disabled));
    assert_eq!(render(&ctx, &mut app).unwrap(), base);
    assert_eq!(app.editor.session.ticket(), ticket);
    assert_eq!(app.editor.session.undo_depth(), 0);
    assert_eq!(app.editor.tools.text_content(), Some("Base"));
}
#[test]
fn source_owner_and_empty_active_lifetime_are_in_the_render_key() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.controls.tool = Tool::Text;
    app.editor.tools.choose_tool(Tool::Text);
    app.sync_text();
    app.editor
        .tools
        .begin_text(app.editor.target.unwrap(), (0, 0));
    app.editor.tools.type_text("Base");
    let original = app.canvas_render_key();
    app.test_start_preedit("候補");
    app.text_event(
        &ctx,
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "".into(),
            active_range_chars: None,
        }),
    );
    assert!(app.text.is_composing());
    assert_ne!(app.canvas_render_key(), original);
    let doc = app.editor.session.document().clone();
    app.editor = Editor::from_document(doc).unwrap();
    app.guard_composition(&ctx);
    assert!(!app.text.is_composing());
    assert_ne!(app.canvas_render_key().owner, original.owner);
}

#[test]
fn real_framework_delete_preedit_batch_never_mutates_base_before_commit() {
    use egui_kittest::Harness;
    let mut h = Harness::builder()
        .with_size(egui::vec2(1360.0, 900.0))
        .build_eframe(|cc| {
            let mut app = WebApp::new(&cc.egui_ctx);
            app.editor = Editor::new(192, 96).unwrap();
            app.controls.tool = Tool::Text;
            app.editor.tools.choose_tool(Tool::Text);
            app.sync_text();
            app.editor
                .tools
                .begin_text(app.editor.target.unwrap(), (8, 8));
            app.editor.tools.type_text("before");
            app
        });
    h.run();
    let stamp = h.state().editor.tools.text_stamp();
    let ticket = h.state().editor.session.ticket();
    // Harness::event steps each queued event in a separate frame. The real
    // text_agent emits both together, so inject one complete RawInput batch.
    h.input_mut().events.extend([
        egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
            before_chars: 2,
            after_chars: 0,
        }),
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "新".into(),
            active_range_chars: None,
        }),
    ]);
    h.run();
    assert_eq!(h.state().editor.tools.text_content(), Some("before"));
    assert_eq!(h.state().editor.tools.text_stamp(), stamp);
    let preview =
        paintcraft_render::render_flat(&h.state().canvas_preview_document().unwrap().unwrap())
            .unwrap();
    h.event(egui::Event::Ime(egui::ImeEvent::Commit("新".into())));
    h.run();
    assert_eq!(h.state().editor.tools.text_content(), Some("befo新"));
    assert_eq!(
        paintcraft_render::render_flat(&h.state().editor.display_document().unwrap().unwrap())
            .unwrap(),
        preview
    );
    assert_eq!(h.state().editor.session.ticket(), ticket);
    assert_eq!(h.state().editor.session.undo_depth(), 0);
}
