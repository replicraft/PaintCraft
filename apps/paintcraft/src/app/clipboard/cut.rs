use super::*;

fn fixture(experience: Experience) -> (App, egui::Context, Fake) {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.settings.experience = experience;
    let doc = paintcraft_document::import_image(3, 2, &[17, 44, 99, 255].repeat(6), "native-cut")
        .unwrap();
    app.editor = Editor::with_document(doc, None);
    app.editor.request(
        1,
        Action::Select {
            corners: Some(((1, 0), (1, 1))),
        },
    );
    app.editor.run_pending();
    let fake = Fake::default();
    app.clipboard.writer = Some(Box::new(fake.clone()));
    (app, ctx, fake)
}
fn cut_key() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::X,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }
}
#[test]
fn cut_waits_for_write_ack_then_removes_only_the_frozen_field_with_one_undo_step() {
    for experience in [Experience::Classic, Experience::Studio] {
        for background in [false, true] {
            let (mut app, ctx, fake) = fixture(experience);
            if background {
                app.context = ctx.clone();
                app.initialize_editing(ctx.clone());
            }
            let before = app.editor.document_snapshot();
            let ticket = app.editor.history_view().ticket;
            let depth = app.editor.history_view().entries.len();
            input(&mut app, &ctx, vec![egui::Event::Cut, cut_key()]);
            wait_write(&mut app.clipboard, &ctx);
            assert_eq!(fake.images.lock().unwrap().len(), 1);
            assert_eq!(
                fake.images.lock().unwrap()[0].rgba(),
                [17, 44, 99, 255].repeat(2)
            );
            assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
            assert_eq!(app.editor.history_view().ticket, ticket);
            assert!(app.clipboard.last_capture.is_none());
            *fake.acknowledgement.lock().unwrap() = Some(Ok(()));
            app.poll_clipboard(&ctx);
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while app.editor.has_pending_edits() {
                app.poll_edits(&ctx);
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            let doc = app.editor.document();
            let raster = &doc.layers[&doc.roots[0]].raster;
            for y in 0..2 {
                for x in 0..3 {
                    let expected = if x == 1 {
                        if experience == Experience::Studio {
                            Rgba::TRANSPARENT
                        } else {
                            Rgba([255; 4])
                        }
                    } else {
                        Rgba([17, 44, 99, 255])
                    };
                    assert_eq!(raster.get(x, y), expected);
                }
            }
            assert_eq!(app.editor.history_view().entries.len(), depth + 1);
            assert_eq!(
                app.editor
                    .history_view()
                    .entries
                    .last()
                    .unwrap()
                    .label
                    .text(),
                "Cut"
            );
            assert!(app.clipboard.last_capture.is_some());
            app.editor.request(999, Action::Undo);
            app.editor.run_pending();
            while app.editor.has_pending_edits() {
                app.poll_edits(&ctx);
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            assert!(app.editor.document().same_semantics(&before));
        }
    }
}
#[test]
fn failed_write_and_escape_never_delete_or_replace_internal_clipboard() {
    for cancel in [false, true] {
        let (mut app, ctx, fake) = fixture(Experience::Classic);
        let before = app.editor.document_snapshot();
        app.cut_requested(&ctx);
        wait_write(&mut app.clipboard, &ctx);
        if cancel {
            input(
                &mut app,
                &ctx,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            *fake.acknowledgement.lock().unwrap() = Some(Ok(()));
        } else {
            *fake.acknowledgement.lock().unwrap() = Some(Err("denied".into()));
        }
        app.poll_clipboard(&ctx);
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert!(app.clipboard.last_capture.is_none());
        assert!(app.clipboard.pending.is_none());
    }
}
#[test]
fn changed_source_target_new_draft_or_new_tab_downgrades_cut_to_copy() {
    for change in 0..5 {
        let (mut app, ctx, fake) = fixture(Experience::Studio);
        app.cut_requested(&ctx);
        wait_write(&mut app.clipboard, &ctx);
        match change {
            0 => {
                app.editor.request(9, Action::Select { corners: None });
                app.editor.run_pending();
            }
            1 => {
                app.editor.request(
                    9,
                    Action::ClearCanvas {
                        color: Rgba([1, 2, 3, 255]),
                    },
                );
                app.editor.run_pending();
            }
            2 => {
                app.editor.pointer_down((0, 0));
            }
            3 => {
                app.add_document_tab(Editor::new_document(2, 2).unwrap(), &ctx)
                    .unwrap();
            }
            _ => {
                app.ime_composing = true;
            }
        }
        let current = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        *fake.acknowledgement.lock().unwrap() = Some(Ok(()));
        app.poll_clipboard(&ctx);
        assert!(Arc::ptr_eq(&current, &app.editor.document_snapshot()));
        assert_eq!(app.editor.history_view().ticket, ticket);
        assert!(app.clipboard.last_capture.is_some());
        assert!(!app.editor.has_pending_edits());
        if change != 3 {
            assert!(app.editor.status().contains("source changed"));
        }
    }
}
