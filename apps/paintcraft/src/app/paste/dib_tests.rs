use super::tests::{fixture, ready};
use super::*;
use paintcraft_platform::clipboard::Encoding;
fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn source(v5: bool, metadata: bool) -> EncodedImage {
    let size = if v5 { 124 } else { 40 };
    let mut bytes = vec![0; size];
    put(&mut bytes, 0, size as u32);
    put(&mut bytes, 4, 3);
    put(&mut bytes, 8, (-2i32) as u32);
    bytes[12..14].copy_from_slice(&1u16.to_le_bytes());
    bytes[14..16].copy_from_slice(&32u16.to_le_bytes());
    if v5 {
        put(&mut bytes, 16, 3);
        for (at, value) in [
            (40, 0xff0000),
            (44, 0xff00),
            (48, 0xff),
            (52, 0xff000000),
            (56, 0x73524742),
        ] {
            put(&mut bytes, at, value);
        }
    }
    if metadata {
        put(&mut bytes, 24, 3780);
    }
    for n in 0..6u8 {
        bytes.extend([90 + n, 40 + n, 10 + n, 0]);
    }
    let length = bytes.len();
    EncodedImage::capture(
        if v5 { Encoding::DibV5 } else { Encoding::Dib },
        length,
        &encoded_budget(),
        &|| Ok(()),
        || Ok(bytes),
    )
    .unwrap()
}
fn settle(app: &mut App) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app.editor.has_pending_edits() {
        app.editor.run_pending();
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
}
#[test]
fn legacy_dib_opaque_alpha_requires_consent_and_places_exact_pixels_once() {
    for frontend in [
        Frontend::ClassicV7,
        Frontend::ClassicV10,
        Frontend::ClassicV11,
        Frontend::Studio,
    ] {
        for asynchronous in [false, true] {
            let (mut app, ctx, fake) = fixture();
            match frontend {
                Frontend::ClassicV7 => app.settings.classic_variant = Some(ClassicVariant::V7),
                Frontend::ClassicV10 => app.settings.classic_variant = Some(ClassicVariant::V10),
                Frontend::Studio => app.settings.experience = Experience::Studio,
                _ => {}
            }
            if asynchronous {
                app.initialize_editing(ctx.clone());
            }
            let original = app.editor.document_snapshot();
            let target = app.editor.target().unwrap();
            *fake.outcome.lock().unwrap() = Some(Ok(source(false, false)));
            app.paste_requested(&ctx);
            ready(&mut app, &ctx);
            let draft = app.paste.ready.take().unwrap();
            assert_eq!(draft.losses, ["dib-alpha-unspecified"]);
            app.commit_ready_paste(&ctx, draft);
            assert!(app.paste.ready.is_some());
            assert!(!app.editor.has_image_paste());
            assert!(Arc::ptr_eq(&original, &app.editor.document_snapshot()));
            let mut draft = app.paste.ready.take().unwrap();
            draft.accept_losses = true;
            draft.origin = (1, 0);
            app.commit_ready_paste(&ctx, draft);
            assert!(app.editor.has_image_paste());
            app.editor.place_image_paste().unwrap();
            settle(&mut app);
            assert!(!app.editor.has_image_paste());
            assert_eq!(app.editor.document().layers.len(), 1);
            assert_eq!(app.editor.undo_depth(), 1);
            for y in 0..2 {
                for x in 0..3 {
                    let n = (y * 3 + x) as u8;
                    assert_eq!(
                        app.editor.document().layers[&target].raster.get(x + 1, y),
                        Rgba([10 + n, 40 + n, 90 + n, 255])
                    );
                }
            }
            let bytes = paintcraft_formats::pcnd::write(app.editor.document()).unwrap();
            let reopened = paintcraft_formats::pcnd::read(&bytes).unwrap();
            assert!(reopened.same_semantics(app.editor.document()));
            app.editor.request(991, Action::Undo);
            app.editor.run_pending();
            settle(&mut app);
            assert!(app.editor.document().same_semantics(&original));
            assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        }
    }
}
#[test]
fn explicit_v5_alpha_is_kept_and_metadata_losses_gate_publication() {
    for metadata in [false, true] {
        let (mut app, ctx, fake) = fixture();
        let original = app.editor.document_snapshot();
        *fake.outcome.lock().unwrap() = Some(Ok(source(true, metadata)));
        app.paste_layer_requested(&ctx);
        ready(&mut app, &ctx);
        let mut draft = app.paste.ready.take().unwrap();
        assert_eq!(draft.draft.image().rgba()[3], 0);
        assert_eq!(
            draft.losses,
            if metadata {
                vec!["dib-alpha-mode-unspecified", "dib-metadata-ignored"]
            } else {
                vec!["dib-alpha-mode-unspecified"]
            }
        );
        if metadata {
            app.commit_ready_paste(&ctx, draft);
            assert!(app.paste.ready.is_some());
            assert!(Arc::ptr_eq(&original, &app.editor.document_snapshot()));
            app.cancel_paste(&ctx);
        } else {
            draft.accept_losses = true;
            app.commit_ready_paste(&ctx, draft);
            assert_eq!(app.editor.document().layers.len(), 2);
            let target = app.editor.target().unwrap();
            assert_eq!(
                app.editor.document().layers[&target].raster.get(0, 0),
                Rgba([10, 40, 90, 0])
            );
        }
    }
}

#[test]
fn dib_loss_consent_is_translated_and_blocks_actual_dialog_actions_until_accepted() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    use paintcraft_ui::i18n::{Language, translate};
    for language in [Language::English, Language::Japanese] {
        let (mut app, _, fake) = fixture();
        app.settings.language = language;
        *fake.outcome.lock().unwrap() = Some(Ok(source(true, true)));
        let original = app.editor.document_snapshot();
        let mut h = Harness::builder()
            .with_size(Vec2::new(1360.0, 900.0))
            .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
        h.run();
        h.input_mut().events.push(egui::Event::Key {
            key: egui::Key::V,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        });
        h.step();
        let checkbox = translate(language, "Accept import limitations");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while h.query_by_label(&checkbox).is_none() {
            assert!(
                std::time::Instant::now() < deadline,
                "{}",
                h.state().editor.status()
            );
            h.step();
            std::thread::yield_now();
        }
        for code in ["dib-alpha-mode-unspecified", "dib-metadata-ignored"] {
            let description = paintcraft_ui::i18n::import_loss_description(code).unwrap();
            assert!(
                h.query_by_label(&translate(language, description))
                    .is_some()
            );
        }
        let paste = translate(language, "Paste");
        assert!(
            !h.query_all_by_label(&paste)
                .any(|n| n.accesskit_node().role() == egui::Role::Button
                    && !n.accesskit_node().is_disabled())
        );
        h.get_by_label(&checkbox).click();
        h.run();
        h.query_all_by_label(&paste)
            .find(|n| {
                n.accesskit_node().role() == egui::Role::Button && !n.accesskit_node().is_disabled()
            })
            .unwrap()
            .click();
        h.run();
        assert!(h.state().editor.has_image_paste());
        assert_eq!(h.state().editor.undo_depth(), 0);
        h.get_by_label(&translate(language, "Cancel")).click();
        h.run();
        assert!(!h.state().editor.has_image_paste());
        assert!(Arc::ptr_eq(
            &original,
            &h.state().editor.document_snapshot()
        ));
    }
}
