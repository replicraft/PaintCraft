use super::*;
use crate::editor::Editor;
use paintcraft_core::types::{Rgba, clipboard::Image, resources::SharedBudget};
use paintcraft_raster::RasterLayer;
use std::sync::Arc;
fn decoded() -> clipboard::Decoded {
    clipboard::Decoded {
        image: Image::from_rgba(
            2,
            1,
            vec![255, 0, 0, 127, 17, 44, 99, 0],
            &SharedBudget::new(8),
            &|| Ok(()),
        )
        .unwrap(),
        losses: vec![],
    }
}
fn fixture(ctx: &egui::Context) -> WebApp {
    let mut app = WebApp::new(ctx);
    let mut doc = paintcraft_document::new_document(
        8,
        6,
        Some(Rgba([0, 0, 255, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let base = doc.roots[0];
    let mut front = doc.layers[&base].clone();
    front.id = uuid::Uuid::new_v4();
    front.raster = RasterLayer::new(2, 1, Rgba([0, 255, 0, 255]));
    doc.roots.push(front.id);
    doc.layers.insert(front.id, front);
    app.editor = Editor::from_document(doc).unwrap();
    app.editor.target = Some(base);
    app
}
fn stage(app: &mut WebApp, ctx: &egui::Context) {
    app.paste.intent = Intent::CurrentLayer;
    app.paste_with(ctx, || Ok(clipboard::Job::completed(Ok(decoded()))));
    app.poll_paste(ctx);
    let ready = app.paste.ready.take().unwrap();
    app.accept_paste(ctx, ready);
    assert!(app.paste.is_placing());
}
#[test]
fn preview_and_multiple_private_positions_match_one_ordered_paste_publication_and_undo() {
    let ctx = egui::Context::default();
    let mut app = fixture(&ctx);
    let before = app.editor.session.snapshot();
    let history = app.editor.session.history_view();
    stage(&mut app, &ctx);
    assert!(app.unsaved());
    assert!(!app.editor.session.is_dirty());
    let first = app.paste_preview_document().unwrap().unwrap();
    assert_eq!(
        &paintcraft_render::render_flat(&first).unwrap()[..4],
        &[0, 255, 0, 255]
    );
    for origin in [(2, 1), (-1, 0), (3, 2)] {
        app.change_paste_position(&ctx, |p| p.place(origin));
        assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
        assert_eq!(app.editor.session.history_view(), history);
    }
    let preview = app.paste_preview_document().unwrap().unwrap();
    let pixels = paintcraft_render::render_flat(&preview).unwrap();
    let at = (2 * 8 + 3) * 4;
    assert_eq!(&pixels[at..at + 4], &[127, 0, 128, 255]);
    assert_eq!(
        app.visible_selection().unwrap(),
        paintcraft_raster::Selection::Rect(Rect::new(3, 2, 2, 1).unwrap())
    );
    app.place_paste(&ctx);
    assert!(!app.paste.busy());
    assert_eq!(app.editor.session.undo_depth(), 1);
    assert_eq!(
        paintcraft_render::render_flat(app.editor.session.document()).unwrap(),
        pixels
    );
    assert_eq!(app.editor.session.document().roots, before.roots);
    app.editor.session.undo().unwrap();
    assert!(app.editor.session.document().same_semantics(&before));
}
#[test]
fn direct_file_output_preserves_private_paste_until_explicit_placement() {
    let ctx = egui::Context::default();
    let mut app = fixture(&ctx);
    let before = app.editor.session.snapshot();
    stage(&mut app, &ctx);
    app.download(&ctx, false);
    assert!(!app.files.busy);
    assert!(app.files.poll().is_none());
    app.download_image(&ctx, paintcraft_ui::actions::ImageExport::Png);
    assert!(!app.files.busy);
    assert!(app.files.poll().is_none());
    assert!(app.paste.is_placing());
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.undo_depth(), 0);
    assert!(app.unsaved());
    assert_eq!(
        app.message,
        "Finish placing the pasted image before editing or saving."
    );
}
#[test]
fn escape_and_undo_cancel_private_paste_without_navigating_existing_history() {
    for key in [egui::Key::Escape, egui::Key::Z] {
        let ctx = egui::Context::default();
        let mut app = fixture(&ctx);
        let before = app.editor.session.snapshot();
        let history = app.editor.session.history_view();
        stage(&mut app, &ctx);
        assert!(app.keyboard_event(
            &ctx,
            &egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                physical_key: None,
                modifiers: if key == egui::Key::Z {
                    egui::Modifiers::COMMAND
                } else {
                    egui::Modifiers::NONE
                }
            }
        ));
        assert!(!app.paste.busy());
        assert!(!app.unsaved());
        assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
        assert_eq!(history, app.editor.session.history_view());
    }
}
#[test]
fn stale_target_tool_and_selection_cancel_without_publishing_and_failed_commit_keeps_pixels() {
    for mode in 0..3 {
        let ctx = egui::Context::default();
        let mut app = fixture(&ctx);
        stage(&mut app, &ctx);
        match mode {
            0 => app.editor.target = Some(app.editor.session.document().roots[1]),
            1 => app.controls.tool = Tool::Brush,
            _ => {
                app.editor
                    .apply(paintcraft_tools::Action::Select {
                        corners: Some(((1, 1), (2, 2))),
                    })
                    .unwrap();
            }
        }
        let before = app.editor.session.snapshot();
        app.validate_paste_placement(&ctx);
        assert!(!app.paste.busy());
        assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    }
    let ctx = egui::Context::default();
    let mut app = fixture(&ctx);
    stage(&mut app, &ctx);
    let before = app.editor.session.snapshot();
    app.editor.session.set_staged_pixel_budget(0);
    app.place_paste(&ctx);
    assert!(app.paste.is_placing());
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    app.editor
        .session
        .set_staged_pixel_budget(crate::editor::MAX_PIXELS * 4);
    app.place_paste(&ctx);
    assert!(!app.paste.busy());
    assert_eq!(app.editor.session.undo_depth(), 1);
}
#[test]
fn canvas_growth_is_private_until_placement_and_invalid_movement_preserves_the_last_position() {
    let ctx = egui::Context::default();
    let mut app = fixture(&ctx);
    app.paste.intent = Intent::CurrentLayer;
    app.paste_with(&ctx, || Ok(clipboard::Job::completed(Ok(decoded()))));
    app.poll_paste(&ctx);
    let mut ready = app.paste.ready.take().unwrap();
    ready.grow = true;
    ready.origin = (8, 6);
    app.accept_paste(&ctx, ready);
    assert_eq!(app.editor.session.document().canvas, (8, 6));
    app.change_paste_position(&ctx, |p| p.place((-1, 0)));
    assert_eq!(
        app.paste.placing.as_ref().unwrap().position.origin(),
        (8, 6)
    );
    app.change_paste_position(&ctx, |p| p.place((4096, 4096)));
    assert_eq!(
        app.paste.placing.as_ref().unwrap().position.origin(),
        (8, 6)
    );
    app.place_paste(&ctx);
    assert_eq!(app.editor.session.document().canvas, (10, 7));
}
#[test]
fn actual_shortcut_modal_drag_release_regrab_and_apply_work_in_each_skin_and_two_languages() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    use paintcraft_ui::i18n::{Language, set_language};
    for variant in ClassicVariant::ALL {
        for language in [Language::English, Language::Japanese] {
            let mut h = Harness::builder()
                .with_size(egui::vec2(1360.0, 900.0))
                .build_eframe(|cc| {
                    let mut app = fixture(&cc.egui_ctx);
                    app.variant = variant;
                    set_language(&cc.egui_ctx, language);
                    app.paste.next_read = Some(clipboard::Job::completed(Ok(decoded())));
                    app
                });
            h.run();
            h.event(egui::Event::Key {
                key: egui::Key::V,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            });
            h.run();
            assert!(h.state().paste.ready.is_some());
            let label = t!(&h.ctx, "Paste");
            h.get_all_by_role_and_label(egui::Role::Button, &label)
                .into_iter()
                .find(|n| !n.accesskit_node().is_disabled())
                .unwrap()
                .click();
            h.run();
            assert!(h.state().paste.is_placing());
            let source = h.state().editor.session.snapshot();
            for (from, to) in [((0.5, 0.5), (2.5, 1.5)), ((2.5, 1.5), (3.5, 2.5))] {
                let r = h.state().canvas.rect.unwrap();
                let z = h.state().zoom;
                let a = r.min + egui::vec2(from.0, from.1) * z;
                let b = r.min + egui::vec2(to.0, to.1) * z;
                h.event(egui::Event::PointerMoved(a));
                h.event(egui::Event::PointerButton {
                    pos: a,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                });
                h.step();
                h.event(egui::Event::PointerMoved(b));
                h.step();
                h.event(egui::Event::PointerButton {
                    pos: b,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                });
                h.run();
                assert!(h.state().paste.is_placing());
                assert!(Arc::ptr_eq(&source, &h.state().editor.session.snapshot()));
            }
            assert_eq!(
                h.state().paste.placing.as_ref().unwrap().position.origin(),
                (3, 2)
            );
            h.get_by_role_and_label(egui::Role::Button, &t!(&h.ctx, "Apply"))
                .click();
            h.run();
            assert!(!h.state().paste.busy());
            assert_eq!(h.state().editor.session.undo_depth(), 1);
        }
    }
}
