use super::*;
use std::sync::Mutex;
#[derive(Clone)]
struct Fake {
    outcome: Arc<Mutex<Option<Result<EncodedImage, String>>>>,
    calls: Arc<std::sync::atomic::AtomicUsize>,
}
impl ReadJob for Fake {
    fn poll(&self) -> Option<Result<EncodedImage, String>> {
        self.outcome.lock().unwrap().take()
    }
}
impl Reader for Fake {
    fn supported(&self) -> bool {
        true
    }
    fn submit(
        &self,
        _budget: SharedBudget,
        _ctx: &egui::Context,
    ) -> Result<Box<dyn ReadJob>, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(self.clone()))
    }
}
fn fixture() -> (App, egui::Context, Fake) {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.editor = Editor::new_document(4, 3).unwrap();
    let fake = Fake {
        outcome: Arc::new(Mutex::new(None)),
        calls: Default::default(),
    };
    app.paste.reader = Some(Box::new(fake.clone()));
    app.paste.raw_budget = SharedBudget::new(128 << 20);
    (app, ctx, fake)
}
fn encoded() -> EncodedImage {
    let bytes = image::encode(
        image::ImageFormat::Png,
        3,
        2,
        &[
            17, 44, 99, 0, 255, 0, 7, 127, 90, 80, 70, 255, 0, 0, 0, 0, 1, 2, 3, 1, 4, 5, 6, 200,
        ],
        90,
    )
    .unwrap();
    EncodedImage::capture(
        paintcraft_platform::clipboard::Encoding::Png,
        bytes.len(),
        &encoded_budget(),
        &|| Ok(()),
        || Ok(bytes),
    )
    .unwrap()
}
fn ready(app: &mut App, ctx: &egui::Context) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app.paste.pending.is_some() {
        app.poll_paste(ctx);
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(app.paste.ready.is_some(), "{}", app.editor.status());
}
#[test]
fn asynchronous_read_decode_and_accepted_growth_make_one_exact_undoable_layer() {
    for background in [false, true] {
        let (mut app, ctx, fake) = fixture();
        if background {
            app.context = ctx.clone();
            app.initialize_editing(ctx.clone());
        }
        app.settings.canvas.remember_last = true;
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let depth = app.editor.history_view().entries.len();
        app.paste_layer_requested(&ctx);
        app.paste_layer_requested(&ctx);
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        *fake.outcome.lock().unwrap() = Some(Ok(encoded()));
        ready(&mut app, &ctx);
        assert_eq!(app.editor.history_view().ticket, ticket);
        let mut ready = app.paste.ready.take().unwrap();
        ready.origin = (3, 2);
        ready.grow = true;
        app.commit_ready_paste(&ctx, ready);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while app.editor.has_pending_edits() {
            app.poll_edits(&ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(app.editor.document().canvas, (6, 4));
        assert_eq!(app.settings.canvas.last, Some((6, 4)));
        assert_eq!(app.editor.document().roots.len(), 2);
        let target = app.editor.target().unwrap();
        assert_eq!(app.editor.document().layers[&target].origin, (3, 2));
        assert_eq!(
            app.editor.document().layers[&target].raster.get(0, 0),
            Rgba([17, 44, 99, 0])
        );
        assert_eq!(
            app.editor.document().layers[&target].raster.get(1, 0),
            Rgba([255, 0, 7, 127])
        );
        assert_eq!(app.editor.history_view().entries.len(), depth + 1);
        assert_eq!(
            app.editor
                .history_view()
                .entries
                .last()
                .unwrap()
                .label
                .text(),
            "Paste"
        );
        app.editor.request(19, Action::Undo);
        app.editor.run_pending();
        while app.editor.has_pending_edits() {
            app.poll_edits(&ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(app.editor.document().same_semantics(&before));
    }
}
#[test]
fn read_failure_cancel_and_late_sources_never_replace_document_or_active_tab_status() {
    for mode in 0..5 {
        let (mut app, ctx, fake) = fixture();
        app.settings.experience = Experience::Studio;
        let original = app.editor.document_snapshot();
        app.paste_layer_requested(&ctx);
        match mode {
            0 => *fake.outcome.lock().unwrap() = Some(Err("denied".into())),
            1 => app.cancel_paste(&ctx),
            2 => {
                app.editor.request(
                    22,
                    Action::Select {
                        corners: Some(((0, 0), (1, 1))),
                    },
                );
                app.editor.run_pending();
            }
            3 => {
                app.add_document_tab(Editor::new_document(2, 2).unwrap(), &ctx)
                    .unwrap();
                app.editor.set_status("other document");
            }
            _ => {
                app.editor.pointer_down((0, 0));
            }
        }
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        if mode >= 2 {
            *fake.outcome.lock().unwrap() = Some(Ok(encoded()));
        }
        app.poll_paste(&ctx);
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert_eq!(app.editor.history_view().ticket, ticket);
        assert!(!app.paste.busy());
        if mode == 3 {
            assert_eq!(app.editor.status(), "other document");
        } else if mode == 0 {
            assert!(app.editor.status().contains("denied"));
        }
        assert_eq!(original.canvas, (4, 3));
    }
}
#[test]
fn unaccepted_losses_invalid_position_and_cancel_leave_ready_pixels_private() {
    let (mut app, ctx, fake) = fixture();
    let before = app.editor.document_snapshot();
    app.paste_layer_requested(&ctx);
    *fake.outcome.lock().unwrap() = Some(Ok(encoded()));
    ready(&mut app, &ctx);
    let mut ready = app.paste.ready.take().unwrap();
    ready.losses = vec!["depth-16-reduced-to-8".into()];
    app.commit_ready_paste(&ctx, ready);
    assert!(app.paste.ready.is_some());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    let mut ready = app.paste.ready.take().unwrap();
    ready.accept_losses = true;
    ready.origin = (-1, 0);
    ready.grow = true;
    app.commit_ready_paste(&ctx, ready);
    assert!(app.paste.ready.is_some());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    app.cancel_paste(&ctx);
    assert!(!app.paste.busy());
    assert!(app.confirm.is_none());
}

#[test]
fn admitted_paste_retries_decoded_capacity_without_rereading_and_escape_can_cancel_it() {
    for cancel in [false, true] {
        let (mut app, ctx, fake) = fixture();
        let bytes = paintcraft_raster::RasterLayer::TILE_BYTES as u64;
        let budget = SharedBudget::new(bytes);
        let blocker = budget.reserve(bytes).unwrap();
        let service =
            controller::background::Service::with_residency(ctx.clone(), budget.clone()).unwrap();
        app.editor.connect_editing(service.clone());
        app.editing = Some(service);
        let before = app.editor.document_snapshot();
        app.paste_layer_requested(&ctx);
        *fake.outcome.lock().unwrap() = Some(Ok(encoded()));
        ready(&mut app, &ctx);
        let ready = app.paste.ready.take().unwrap();
        app.commit_ready_paste(&ctx, ready);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while app.editor.allocation_wait().is_none() {
            app.poll_edits(&ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        assert!(app.paste.raw_budget.usage().reserved > 0);
        if cancel {
            app.editor.cancel_pending_edits();
            assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
            assert_eq!(app.paste.raw_budget.usage().reserved, 0);
        } else {
            drop(blocker);
            while app.editor.has_pending_edits() {
                app.poll_edits(&ctx);
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            assert_eq!(app.editor.document().roots.len(), 2);
            assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
            assert_eq!(app.paste.raw_budget.usage().reserved, 0);
            continue;
        }
        drop(blocker);
    }
}
#[test]
fn malformed_encoded_data_and_raw_budget_denial_do_not_stage_or_publish_pixels() {
    for malformed in [false, true] {
        let (mut app, ctx, fake) = fixture();
        let before = app.editor.document_snapshot();
        if !malformed {
            app.paste.raw_budget = SharedBudget::new(0);
        }
        app.paste_layer_requested(&ctx);
        let encoded = if malformed {
            let data = b"\x89PNG\r\n\x1a\nmalformed".to_vec();
            EncodedImage::capture(
                paintcraft_platform::clipboard::Encoding::Png,
                data.len(),
                &encoded_budget(),
                &|| Ok(()),
                || Ok(data),
            )
            .unwrap()
        } else {
            encoded()
        };
        *fake.outcome.lock().unwrap() = Some(Ok(encoded));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while app.paste.pending.is_some() {
            app.poll_paste(&ctx);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(app.paste.ready.is_none());
        assert!(app.editor.status().contains("Clipboard paste failed"));
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert_eq!(app.paste.raw_budget.usage().reserved, 0);
    }
}
#[test]
fn actual_controls_and_shortcut_stage_then_commit_or_cancel_in_each_skin_and_two_languages() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    use paintcraft_ui::i18n::{Language, translate};
    for (experience, skin) in [
        (Experience::Classic, ClassicVariant::V7),
        (Experience::Classic, ClassicVariant::V10),
        (Experience::Classic, ClassicVariant::V11),
        (Experience::Studio, ClassicVariant::V11),
    ] {
        for language in [Language::English, Language::Japanese] {
            let (mut app, _, fake) = fixture();
            app.settings.experience = experience;
            app.settings.classic_variant = Some(skin);
            app.settings.language = language;
            *fake.outcome.lock().unwrap() = Some(Ok(encoded()));
            let before = app.editor.document_snapshot();
            let mut h = Harness::builder()
                .with_size(Vec2::new(1360.0, 900.0))
                .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
            h.run();
            if experience == Experience::Classic && skin != ClassicVariant::V11 {
                h.input_mut().events.push(egui::Event::Key {
                    key: egui::Key::V,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                });
                h.run();
            } else {
                let edit = translate(language, "Edit");
                h.get_by_label(&edit).click();
                h.run();
                let label = translate(language, "Paste as new layer…");
                let control = h
                    .query_all_by_label_contains(&label)
                    .find(|node| !node.accesskit_node().is_disabled())
                    .expect("enabled Paste as new layer");
                control.click();
                h.run();
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while h.state().paste.ready.is_none() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "{}",
                    h.state().editor.status()
                );
                h.step();
                std::thread::yield_now();
            }
            assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
            let cancel = skin == ClassicVariant::V7;
            let label = translate(language, if cancel { "Cancel" } else { "Paste" });
            h.query_all_by_label(&label)
                .find(|node| !node.accesskit_node().is_disabled())
                .expect("enabled dialog action")
                .click();
            h.run();
            if cancel {
                assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
                assert!(!h.state().paste.busy());
            } else {
                assert_eq!(h.state().editor.document().roots.len(), 2);
                assert_eq!(
                    h.state()
                        .editor
                        .history_view()
                        .entries
                        .last()
                        .unwrap()
                        .label
                        .text(),
                    "Paste"
                );
            }
            assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        }
    }
}

#[test]
fn private_gestures_text_and_pending_edits_refuse_clipboard_access_without_flushing() {
    for mode in 0..3 {
        let (mut app, ctx, fake) = fixture();
        match mode {
            0 => app.editor.pointer_down((0, 0)),
            1 => {
                app.editor.tool = Tool::Text;
                app.editor.pointer_down((0, 0));
                app.editor.pointer_up();
                app.editor.type_text("keep this text");
            }
            _ => {
                app.editor.request(
                    91,
                    Action::ClearCanvas {
                        color: Rgba([1, 2, 3, 255]),
                    },
                );
            }
        }
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        app.dispatch_chrome(&ctx, vec![paintcraft_ui::actions::Action::PasteLayer]);
        assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert_eq!(ticket, app.editor.history_view().ticket);
        if mode == 2 {
            assert!(app.editor.has_pending_edits());
        }
    }
}
