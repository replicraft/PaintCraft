use super::*;

#[test]
fn ctrl_s_saves_safely_and_clears_the_unsaved_marker() {
    let dir = scratch("save");
    let file = dir.join("work.pcnd");
    let mut h = harness_with(App::new(Some(file.clone())), 1.0, false);
    with_layer(&mut h);
    let p = at(&h, 3.0, 3.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(p), press(p), release(p)]);
    h.step();
    h.run();
    assert!(h.state().editor().is_dirty());
    h.event(key(Key::S, Modifiers::COMMAND));
    h.step();
    h.run();
    assert!(
        !h.state().editor().is_dirty(),
        "{}",
        h.state().editor().status()
    );
    let reopened = paintcraft::controller::Editor::open(&file).unwrap();
    assert!(
        reopened
            .document()
            .same_semantics(h.state().editor().document())
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn saving_over_an_unrelated_existing_file_needs_confirmation() {
    let dir = scratch("replace");
    let file = dir.join("other.pcnd");
    std::fs::write(&file, b"precious").unwrap();
    // The path names a file that is not a valid document, so a fresh document is shown unbound.
    let mut h = harness_with(App::new(Some(file.clone())), 1.0, false);
    h.get_by_label("Save").click();
    h.run();
    assert_eq!(
        std::fs::read(&file).unwrap(),
        b"precious",
        "nothing is written before confirmation"
    );
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(std::fs::read(&file).unwrap(), b"precious");
    h.get_by_label("Save").click();
    h.run();
    h.get_by_label("Continue").click();
    h.run();
    assert_ne!(std::fs::read(&file).unwrap(), b"precious");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn new_document_asks_before_discarding_unsaved_work() {
    let mut h = harness();
    with_layer(&mut h);
    let id = h.state().editor().document().document_id;
    h.get_by_label("New").click();
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(
        h.state().editor().layers().len(),
        2,
        "cancelled: nothing discarded"
    );
    h.get_by_label("New").click();
    h.run();
    h.get_by_label("Continue").click();
    h.run();
    assert_eq!(h.state().editor().layers().len(), 1);
    assert_eq!(
        h.state().editor().document().document_id,
        id,
        "same deterministic parameters give the same identity"
    );
}

fn app_with_recovery(dir: &std::path::Path) -> App {
    App::new(None)
        .with_recovery(dir.to_path_buf())
        .autosave_every(std::time::Duration::ZERO)
}

#[test]
fn unsaved_work_survives_a_crash_and_is_offered_on_the_next_start() {
    let dir = scratch("recover");
    {
        let mut h = harness_with(app_with_recovery(&dir), 1.0, false);
        with_layer(&mut h);
        drag(&mut h, (10.0, 10.0), (50.0, 10.0));
        h.run_steps(3);
        assert!(
            h.state().editor().has_checkpoint(),
            "autosave wrote a checkpoint"
        );
        // The process "crashes": nothing is saved or cleared; the app is simply dropped.
    }
    let mut h = harness_with(app_with_recovery(&dir), 1.0, false);
    h.get_by_label("Recover").click();
    h.run();
    assert!(
        h.state().editor().is_dirty(),
        "recovered work is unsaved work"
    );
    assert_eq!(h.state().editor().layers().len(), 2);
    let top = h.state().editor().document().roots[1];
    assert_eq!(
        h.state().editor().document().layers[&top]
            .raster
            .get(30, 10)
            .a(),
        255
    );
    assert!(
        paintcraft_formats::recovery::Recovery::scan(&dir).is_empty(),
        "the offer is consumed"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn declining_recovery_discards_it_for_good() {
    let dir = scratch("decline");
    {
        let mut h = harness_with(app_with_recovery(&dir), 1.0, false);
        with_layer(&mut h);
        h.run_steps(3);
    }
    let mut h = harness_with(app_with_recovery(&dir), 1.0, false);
    h.get_by_label("Discard").click();
    h.run();
    assert!(paintcraft_formats::recovery::Recovery::scan(&dir).is_empty());
    drop(h);
    assert!(paintcraft_formats::recovery::Recovery::scan(&dir).is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn closing_the_window_with_unsaved_work_asks_first_and_keeps_the_checkpoint() {
    let dir = scratch("quit");
    let mut h = harness_with(app_with_recovery(&dir), 1.0, false);
    with_layer(&mut h);
    h.input_mut()
        .viewports
        .entry(eframe::egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(eframe::egui::ViewportEvent::Close);
    h.step();
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().editor().is_dirty() && h.state().editor().has_checkpoint());
    h.input_mut()
        .viewports
        .entry(eframe::egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(eframe::egui::ViewportEvent::Close);
    h.step();
    h.run();
    h.get_by_label("Quit anyway").click();
    h.run();
    drop(h);
    assert_eq!(
        paintcraft_formats::recovery::Recovery::scan(&dir).len(),
        1,
        "quitting anyway still leaves a recovery copy"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_clean_document_closes_without_a_prompt_or_leftovers() {
    let dir = scratch("clean");
    let mut h = harness_with(app_with_recovery(&dir), 1.0, false);
    h.input_mut()
        .viewports
        .entry(eframe::egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(eframe::egui::ViewportEvent::Close);
    h.step();
    h.run();
    assert!(h.query_by_label("Quit anyway").is_none());
    drop(h);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn opening_a_picture_imports_it_and_exporting_names_the_losses() {
    use paintcraft_formats::image::{ImageFormat, encode};
    let dir = scratch("images");
    let px: Vec<u8> = (0..16 * 8)
        .flat_map(|i| [(i * 3 % 256) as u8, 80, 200, 255])
        .collect();
    let src = dir.join("pic.png");
    std::fs::write(&src, encode(ImageFormat::Png, 16, 8, &px, 90).unwrap()).unwrap();
    let mut h = harness();
    h.state_mut().set_path_text(&src.to_string_lossy());
    h.get_by_label("Open").click();
    h.run();
    // The new document has no unsaved work yet, so there is nothing to discard... except the fresh one's layer.
    if h.query_by_label("Continue").is_some() {
        h.get_by_label("Continue").click();
        h.run();
    }
    let doc = h.state().editor().document();
    assert_eq!(doc.canvas, (16, 8));
    assert_eq!(
        h.state().rendered(),
        px.as_slice(),
        "the picture is shown exactly"
    );

    // JPEG export of this picture is lossy: the dialog says so and only then writes.
    let out = dir.join("out.jpg");
    h.state_mut().set_path_text(&out.to_string_lossy());
    h.get_by_label("Export").click();
    h.run();
    assert!(!out.exists(), "nothing is written before confirmation");
    h.get_by_label("Cancel").click();
    h.run();
    assert!(!out.exists());
    h.get_by_label("Export").click();
    h.run();
    h.get_by_label_contains("lossy-jpeg");
    h.get_by_label("Export now").click();
    h.run();
    assert!(out.exists());
    let back = paintcraft_formats::image::decode(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!((back.width, back.height), (16, 8));

    h.state_mut().set_path_text("noextension");
    h.get_by_label("Export").click();
    h.run();
    assert!(h.state().editor().status().contains(".png"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn recovery_offer_asks_before_replacing_edits_made_in_the_current_window() {
    use paintcraft_formats::recovery::Recovery;
    let dir = scratch("recover-over-edits");
    let original = paintcraft_document::new_document(
        4,
        4,
        Some(Rgba([7, 8, 9, 255])),
        73,
        paintcraft_document::POLICY,
    )
    .unwrap();
    {
        let owner = Recovery::start(&dir).unwrap();
        owner.checkpoint_document(&original, None).unwrap();
    }
    let mut h = harness_with(app_with_recovery(&dir), 1.0, false);
    with_layer(&mut h);
    let current = h.state().editor().document_snapshot();
    h.get_by_label("Recover").click();
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&current));
    assert_eq!(
        Recovery::scan(&dir).len(),
        1,
        "cancellation retains the orphan"
    );
    h.get_by_label("Recover").click();
    h.run();
    h.get_by_label("Continue").click();
    h.run();
    assert!(h.state().editor().document().same_semantics(&original));
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}
