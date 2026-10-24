use super::*;
use std::sync::Mutex;

#[derive(Clone, Default)]
struct Fake {
    images: Arc<Mutex<Vec<Image>>>,
    acknowledgement: Arc<Mutex<Option<Result<(), String>>>>,
}
impl WriteJob for Fake {
    fn poll(&self) -> Option<Result<(), String>> {
        self.acknowledgement.lock().unwrap().take()
    }
}
impl Writer for Fake {
    fn submit(&self, image: Image, _: &egui::Context) -> Result<Box<dyn WriteJob>, String> {
        self.images.lock().unwrap().push(image);
        Ok(Box::new(self.clone()))
    }
}

fn wait_write(state: &mut State, ctx: &egui::Context) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !matches!(
        state.pending.as_ref().map(|p| &p.stage),
        Some(Stage::Write { .. })
    ) {
        assert!(std::time::Instant::now() < deadline);
        assert!(state.poll(ctx).is_none());
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn asynchronous_capture_freezes_pixels_and_retains_internal_capture_only_after_ack() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    let fake = Fake::default();
    app.clipboard.writer = Some(Box::new(fake.clone()));
    let source = app.editor.document_snapshot();
    let owner = app.active_tab_id();
    let publications = app.editor.publications();
    let selection = app.editor.selection();
    app.copy_requested(&ctx);
    assert!(app.clipboard.pending.is_some());
    assert!(app.clipboard.last_capture.is_none());
    // Independently mutate another owned revision while the original Arc is pinned.
    let mut other = (*source).clone();
    other.canvas = (1, 1);
    wait_write(&mut app.clipboard, &ctx);
    assert!(app.clipboard.last_capture.is_none());
    let images = fake.images.lock().unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!((images[0].width(), images[0].height()), source.canvas);
    assert_eq!(&images[0].rgba()[..4], &[255, 255, 255, 255]);
    drop(images);
    assert!(Arc::ptr_eq(&source, &app.editor.document_snapshot()));
    assert_eq!(app.editor.selection(), selection);
    assert_eq!(app.editor.publications(), publications);
    *fake.acknowledgement.lock().unwrap() = Some(Ok(()));
    assert_eq!(app.clipboard.poll(&ctx), Some((owner, Ok(true))));
    assert!(app.clipboard.last_capture.is_some());
    assert_eq!(other.canvas, (1, 1));
}

#[test]
fn failure_does_not_replace_last_capture_and_single_inflight_deduplicates() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    let fake = Fake::default();
    app.clipboard.writer = Some(Box::new(fake.clone()));
    let previous = Capture {
        image: Image::from_rgba(1, 1, vec![7, 8, 9, 0], &SharedBudget::new(4), &|| Ok(())).unwrap(),
        origin: (9, 8),
        selection: Selection::Inactive,
    };
    app.clipboard.last_capture = Some(previous);
    app.copy_requested(&ctx);
    app.copy_requested(&ctx);
    wait_write(&mut app.clipboard, &ctx);
    assert_eq!(fake.images.lock().unwrap().len(), 1);
    *fake.acknowledgement.lock().unwrap() = Some(Err("denied".into()));
    app.poll_clipboard(&ctx);
    assert!(app.editor.status().contains("denied"));
    assert_eq!(
        app.clipboard.last_capture.as_ref().unwrap().image.rgba(),
        &[7, 8, 9, 0]
    );
    assert_eq!(app.editor.publications(), 0);
}

#[test]
fn later_edit_cannot_change_captured_pixels_or_be_undone_by_copy_completion() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.editor = Editor::new_document(3, 2).unwrap();
    let fake = Fake::default();
    app.clipboard.writer = Some(Box::new(fake.clone()));
    app.copy_requested(&ctx);
    app.editor.request(
        909,
        Action::ClearCanvas {
            color: Rgba([1, 2, 3, 0]),
        },
    );
    app.editor.run_pending();
    let after_edit = app.editor.document_snapshot();
    let publications = app.editor.publications();
    wait_write(&mut app.clipboard, &ctx);
    assert_eq!(fake.images.lock().unwrap()[0].rgba(), &[255; 24]);
    *fake.acknowledgement.lock().unwrap() = Some(Ok(()));
    app.poll_clipboard(&ctx);
    assert!(Arc::ptr_eq(&after_edit, &app.editor.document_snapshot()));
    assert_eq!(app.editor.publications(), publications);
    assert_eq!(publications, 1);
}

#[test]
fn pending_edit_and_live_gesture_are_rejected_without_flushing_or_cancelling() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.editor.request(
        910,
        Action::ClearCanvas {
            color: Rgba::TRANSPARENT,
        },
    );
    let before = app.editor.document_snapshot();
    app.copy_requested(&ctx);
    assert!(app.editor.has_pending_edits());
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert!(app.clipboard.pending.is_none());
    app.editor.cancel_pending_edits();
    app.editor.pointer_down((1, 1));
    assert!(app.editor.gesture_in_progress());
    app.copy_requested(&ctx);
    assert!(app.editor.gesture_in_progress());
    assert!(app.clipboard.pending.is_none());
    assert_eq!(app.editor.publications(), 0);
}

#[test]
fn cancelled_capture_does_not_submit_or_publish_and_empty_is_not_written() {
    let ctx = egui::Context::default();
    let app = App::new(None);
    let fake = Fake::default();
    let mut state = State {
        writer: Some(Box::new(fake.clone())),
        ..Default::default()
    };
    state
        .start(
            app.active_tab_id(),
            app.editor.document_snapshot(),
            app.editor.selection(),
            app.editor.target().unwrap(),
            &ctx,
        )
        .unwrap();
    let cancelled = state.pending.as_ref().unwrap().cancelled.clone();
    drop(state.pending.take());
    assert!(cancelled.load(Ordering::Relaxed));
    assert!(state.poll(&ctx).is_none());
    assert!(fake.images.lock().unwrap().is_empty());
    state
        .start(
            app.active_tab_id(),
            app.editor.document_snapshot(),
            Selection::Empty,
            app.editor.target().unwrap(),
            &ctx,
        )
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some((_, result)) = state.poll(&ctx) {
            assert_eq!(result, Ok(false));
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(fake.images.lock().unwrap().is_empty());
}

#[test]
fn owner_status_and_draft_rejection_leave_current_document_untouched() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    let fake = Fake::default();
    app.clipboard.writer = Some(Box::new(fake.clone()));
    let owner = app.active_tab_id();
    app.copy_requested(&ctx);
    wait_write(&mut app.clipboard, &ctx);
    app.add_document_tab(Editor::new_document(2, 2).unwrap(), &ctx)
        .unwrap();
    assert_ne!(app.active_tab_id(), owner);
    app.editor.set_status("other document");
    *fake.acknowledgement.lock().unwrap() = Some(Ok(()));
    app.poll_clipboard(&ctx);
    assert_eq!(app.editor.status(), "other document");
    let parked = app
        .tabs
        .iter()
        .find(|tab| tab.id == owner)
        .unwrap()
        .state
        .as_ref()
        .unwrap();
    assert!(parked.editor.status().contains("Copied"));
    app.ime_composing = true;
    app.copy_requested(&ctx);
    assert!(app.clipboard.pending.is_none());
    assert!(app.editor.status().contains("Finish"));
    assert_eq!(app.editor.publications(), 0);
}

fn input(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| app.handle_global_input(ui),
    );
    output.textures_delta.clear();
}

fn copy_key() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::C,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }
}

#[test]
fn standalone_and_coalesced_copy_events_submit_once_and_text_drafts_keep_keyboard_ownership() {
    for events in [vec![egui::Event::Copy], vec![egui::Event::Copy, copy_key()]] {
        let ctx = egui::Context::default();
        let mut app = App::new(None);
        let fake = Fake::default();
        app.clipboard.writer = Some(Box::new(fake.clone()));
        input(&mut app, &ctx, events);
        wait_write(&mut app.clipboard, &ctx);
        assert_eq!(fake.images.lock().unwrap().len(), 1);
        assert_eq!(app.editor.publications(), 0);
    }
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.editor.tool = Tool::Text;
    app.editor.pointer_down((2, 2));
    app.editor.pointer_up();
    app.editor.type_text("copy this text");
    assert!(app.editor.is_editing_text());
    app.editor.set_status("text owns copy");
    input(&mut app, &ctx, vec![egui::Event::Copy, copy_key()]);
    assert_eq!(app.editor.status(), "text owns copy");
    assert!(app.editor.is_editing_text());
    assert!(app.clipboard.pending.is_none());
    assert_eq!(app.editor.publications(), 0);
}

#[test]
fn copy_controls_work_in_every_native_frontend_and_two_languages_with_acknowledged_status() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT, Queryable};
    use paintcraft_ui::i18n::{Language, translate};
    for (experience, skin) in [
        (Experience::Classic, ClassicVariant::V7),
        (Experience::Classic, ClassicVariant::V10),
        (Experience::Classic, ClassicVariant::V11),
        (Experience::Studio, ClassicVariant::V11),
    ] {
        for language in [Language::English, Language::Japanese] {
            let fake = Fake::default();
            let mut app = App::new(None);
            app.settings.experience = experience;
            app.settings.classic_variant = Some(skin);
            app.settings.language = language;
            app.clipboard.writer = Some(Box::new(fake.clone()));
            let source = app.editor.document_snapshot();
            let before = app.editor.history_view().ticket;
            let mut h = Harness::builder()
                .with_size(Vec2::new(1360.0, 900.0))
                .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
            h.run();
            let edit = translate(language, "Edit");
            if let Some(menu) = h.query_by_label(&edit) {
                menu.hover();
                h.run();
                h.get_by_label(&edit).click();
                h.run();
            }
            let copy = translate(language, "Copy");
            // Enabled menu atom labels also include the formatted shortcut.
            let control = h
                .query_all_by_label_contains(&copy)
                .find(|node| !node.accesskit_node().is_disabled())
                .unwrap_or_else(|| {
                    panic!("Missing enabled Copy: {experience:?} {skin:?} {language:?}")
                });
            assert!(
                !control.accesskit_node().is_disabled(),
                "{experience:?} {skin:?} {language:?}"
            );
            control.click();
            h.run();
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while fake.images.lock().unwrap().is_empty() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
                h.step();
            }
            assert!(h.state().clipboard.last_capture.is_none());
            assert_eq!(fake.images.lock().unwrap().len(), 1);
            assert_eq!(h.state().editor.history_view().ticket, before);
            assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
            *fake.acknowledgement.lock().unwrap() = Some(Ok(()));
            h.step();
            assert_eq!(
                h.state().editor.status(),
                translate(language, "Copied to clipboard.")
            );
            assert!(h.state().clipboard.last_capture.is_some());
            assert_eq!(h.state().editor.history_view().ticket, before);
        }
    }
}
