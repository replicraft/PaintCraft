use super::*;
use paintcraft_core::types::Rgba;
fn fixture() -> (WebApp, egui::Context) {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.editor = crate::editor::Editor::from_document(
        paintcraft_document::import_image(3, 2, &[17, 44, 99, 255].repeat(6), "web-cut").unwrap(),
    )
    .unwrap();
    app.editor
        .apply(Edit::Select {
            corners: Some(((1, 0), (1, 1))),
        })
        .unwrap();
    (app, ctx)
}
#[test]
fn acknowledged_cut_clears_exact_source_in_one_undoable_edit() {
    let (mut app, ctx) = fixture();
    let before = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    let depth = app.editor.session.undo_depth();
    app.cut_with(&ctx, |image| {
        assert_eq!(image.rgba(), [17, 44, 99, 255].repeat(2));
        Ok(clipboard::Job::completed(Ok(())))
    });
    app.cut_with(&ctx, |_| {
        panic!("coalesced Cut activation must not submit twice")
    });
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.ticket(), ticket);
    app.poll_copy(&ctx);
    assert_eq!(app.editor.session.undo_depth(), depth + 1);
    assert_eq!(
        app.editor
            .session
            .history_view()
            .entries
            .last()
            .unwrap()
            .label
            .text(),
        "Cut"
    );
    assert_eq!(app.message, "Cut");
    let doc = app.editor.session.document();
    for y in 0..2 {
        for x in 0..3 {
            assert_eq!(
                doc.layers[&doc.roots[0]].raster.get(x, y),
                if x == 1 {
                    Rgba([255; 4])
                } else {
                    Rgba([17, 44, 99, 255])
                }
            );
        }
    }
    assert!(app.clipboard.last.is_some());
    app.editor.session.undo().unwrap().unwrap();
    assert!(app.editor.session.document().same_semantics(&before));
}
#[test]
fn failure_cancellation_stale_source_draft_and_quota_denial_do_not_remove_pixels() {
    for mode in 0..6 {
        let (mut app, ctx) = fixture();
        app.cut_with(&ctx, |_| {
            Ok(clipboard::Job::completed(if mode == 0 {
                Err(Error::new(ErrorCode::CapabilityUnsupported, "denied"))
            } else {
                Ok(())
            }))
        });
        match mode {
            1 => app.cancel_clipboard_cut(&ctx),
            2 => {
                app.editor.apply(Edit::Select { corners: None }).unwrap();
            }
            3 => {
                app.editor
                    .tools
                    .pointer_down(app.editor.target.unwrap(), (0, 0));
            }
            4 => app.editor.session.set_staged_pixel_budget(0),
            5 => {
                app.editor =
                    crate::editor::Editor::from_document((*app.editor.session.snapshot()).clone())
                        .unwrap();
            }
            _ => {}
        }
        let before = app.editor.session.snapshot();
        let ticket = app.editor.session.ticket();
        let depth = app.editor.session.undo_depth();
        app.poll_copy(&ctx);
        assert!(
            Arc::ptr_eq(&before, &app.editor.session.snapshot()),
            "mode {mode}"
        );
        assert_eq!(app.editor.session.ticket(), ticket);
        assert_eq!(app.editor.session.undo_depth(), depth);
        assert_eq!(app.clipboard.last.is_some(), mode >= 2);
        if mode == 4 {
            assert!(app.message.contains("cut failed"), "{}", app.message);
        }
    }
}
#[test]
fn text_drafts_retain_cut_keyboard_ownership() {
    let (mut app, ctx) = fixture();
    app.editor.tools.activate_tool(Tool::Text);
    app.editor
        .tools
        .begin_text(app.editor.target.unwrap(), (0, 0));
    assert!(!app.keyboard_event(&ctx, &egui::Event::Cut));
    app.cut_with(&ctx, |_| panic!("text draft must not be cut as pixels"));
    assert!(app.clipboard.job.is_none());
    assert!(app.editor.tools.is_editing_text());
}
