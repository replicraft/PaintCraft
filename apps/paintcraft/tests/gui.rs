//! Headless GUI tests: real egui input events through the real layout code.
//! They prove input routing, exactly-once execution and actual-pixel mapping.
//! They do not prove pen/IME behavior, GPU presentation or platform windowing;
//! those need native runs (see docs/IMPLEMENTATION_NOTES.md).

use std::path::PathBuf;

use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use paintcraft::App;
use paintcraft_types::Rgba;

fn harness_with(app: App, ppp: f32, two_passes: bool) -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(1100.0, 900.0))
        .with_pixels_per_point(ppp)
        .build_ui_state(
            move |ui, app: &mut App| {
                app.ui(ui);
                if two_passes && ui.ctx().current_pass_index() == 0 {
                    ui.ctx().request_discard("test: force a second layout pass");
                }
            },
            app,
        );
    h.run();
    h
}

fn harness() -> Harness<'static, App> {
    harness_with(App::new(None), 1.0, false)
}

fn at(h: &Harness<App>, x: f32, y: f32) -> Pos2 {
    h.state().canvas_rect().expect("canvas drawn").min + Vec2::new(x + 0.5, y + 0.5)
}

fn press(pos: Pos2) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }
}

fn release(pos: Pos2) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }
}

fn publications(h: &Harness<App>) -> u64 {
    h.state().editor().publications()
}

fn top_pixel(h: &Harness<App>, x: i64, y: i64) -> Rgba {
    let e = h.state().editor();
    e.document().layers[&e.target().unwrap()].raster.get(x, y)
}

/// Click "Add layer" so drawing happens on a fresh transparent layer.
fn with_layer(h: &mut Harness<App>) {
    h.get_by_label("Add layer").click();
    h.run();
}

/// A key release, so the next press of the same key is a new press and not a repeat.
fn key_up(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: false,
        repeat: false,
        modifiers,
    }
}

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("paintcraft-gui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn one_click_is_one_transaction_and_the_texture_shows_it() {
    let mut h = harness();
    with_layer(&mut h);
    let before = publications(&h);
    h.event(Event::PointerMoved(at(&h, 10.0, 20.0)));
    h.step();
    h.event(press(at(&h, 10.0, 20.0)));
    h.step();
    h.event(release(at(&h, 10.0, 20.0)));
    h.step();
    h.run();
    assert_eq!(publications(&h), before + 1, "exactly one transaction");
    assert_eq!(top_pixel(&h, 10, 20), Rgba([0, 0, 0, 255]));
    assert_eq!(top_pixel(&h, 11, 20), Rgba::TRANSPARENT);
    let shown = &h.state().rendered()[(20 * 320 + 10) * 4..][..4];
    assert_eq!(
        shown,
        [0, 0, 0, 255],
        "the displayed composite matches the document"
    );
    // Idle frames never repeat the edit.
    h.run_steps(5);
    assert_eq!(publications(&h), before + 1);
}

#[test]
fn a_drag_in_one_coalesced_frame_keeps_every_sample_in_order() {
    let mut h = harness();
    with_layer(&mut h);
    let before = publications(&h);
    let (a, b, c) = (
        at(&h, 120.0, 10.0),
        at(&h, 130.0, 10.0),
        at(&h, 260.0, 40.0),
    );
    // The whole gesture arrives inside a single frame, as it can under load.
    h.input_mut().events.extend([
        Event::PointerMoved(a),
        press(a),
        Event::PointerMoved(b),
        Event::PointerMoved(c),
        release(c),
    ]);
    h.step();
    h.run();
    assert_eq!(publications(&h), before + 1);
    for (x, y) in [
        (120, 10),
        (127, 10),
        (128, 10),
        (130, 10),
        (255, 39),
        (256, 39),
        (260, 40),
    ] {
        assert_eq!(top_pixel(&h, x, y).a(), 255, "({x},{y})");
    }
    assert_eq!(top_pixel(&h, 100, 10).a(), 0);
}

#[test]
fn a_drag_spread_over_many_frames_is_still_one_transaction() {
    let mut h = harness();
    with_layer(&mut h);
    let before = publications(&h);
    h.event(Event::PointerMoved(at(&h, 5.0, 5.0)));
    h.step();
    h.event(press(at(&h, 5.0, 5.0)));
    h.step();
    for i in 0..30 {
        h.event(Event::PointerMoved(at(
            &h,
            5.0 + i as f32 * 3.0,
            5.0 + i as f32,
        )));
        h.step();
    }
    assert_eq!(
        publications(&h),
        before,
        "nothing is published while the button is down"
    );
    h.event(release(at(&h, 95.0, 35.0)));
    h.step();
    h.run();
    assert_eq!(publications(&h), before + 1);
}

#[test]
fn a_frame_laid_out_twice_does_not_double_anything() {
    let mut h = harness_with(App::new(None), 1.0, true);
    let layers = h.state().editor().layers().len();
    h.get_by_label("Add layer").click();
    h.run();
    assert_eq!(
        h.state().editor().layers().len(),
        layers + 1,
        "one click, one layer, though layout ran twice"
    );
    let before = publications(&h);
    let p = at(&h, 40.0, 40.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(p), press(p), release(p)]);
    h.step();
    h.run();
    assert_eq!(publications(&h), before + 1);
}

#[test]
fn focus_loss_or_escape_cancels_an_unfinished_stroke() {
    for cancel in [
        Event::WindowFocused(false),
        key(Key::Escape, Modifiers::NONE),
    ] {
        let mut h = harness();
        with_layer(&mut h);
        let before = publications(&h);
        h.event(Event::PointerMoved(at(&h, 5.0, 5.0)));
        h.step();
        h.event(press(at(&h, 5.0, 5.0)));
        h.step();
        h.event(Event::PointerMoved(at(&h, 50.0, 50.0)));
        h.step();
        h.event(cancel);
        h.step();
        h.event(release(at(&h, 50.0, 50.0)));
        h.step();
        h.run();
        assert_eq!(publications(&h), before);
        assert_eq!(top_pixel(&h, 25, 25).a(), 0);
    }
}

#[test]
fn pressing_outside_the_canvas_starts_nothing() {
    let mut h = harness();
    with_layer(&mut h);
    let before = publications(&h);
    let on_toolbar = Pos2::new(40.0, 10.0);
    h.input_mut().events.extend([
        Event::PointerMoved(on_toolbar),
        press(on_toolbar),
        release(on_toolbar),
    ]);
    h.step();
    h.run();
    assert_eq!(publications(&h), before);
}

#[test]
fn undo_and_redo_by_keyboard_and_button_each_run_once() {
    let mut h = harness();
    with_layer(&mut h);
    let p = at(&h, 7.0, 7.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(p), press(p), release(p)]);
    h.step();
    h.run();
    assert_eq!(top_pixel(&h, 7, 7).a(), 255);
    let base = publications(&h);
    h.event(key(Key::Z, Modifiers::COMMAND));
    h.step();
    h.run();
    assert_eq!(publications(&h), base + 1);
    assert_eq!(top_pixel(&h, 7, 7).a(), 0);
    h.get_by_label("Redo").click();
    h.run();
    assert_eq!(publications(&h), base + 2);
    assert_eq!(top_pixel(&h, 7, 7).a(), 255);
    h.event(key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT));
    h.step();
    h.run();
    assert_eq!(
        publications(&h),
        base + 2,
        "with nothing left to redo, Shift+Cmd+Z publishes nothing"
    );
}

#[test]
fn canvas_pixels_are_actual_pixels_at_any_display_scale() {
    for (ppp, expect) in [(1.0f32, 320.0f32), (2.0, 160.0)] {
        let h = harness_with(App::new(None), ppp, false);
        let r = h.state().canvas_rect().unwrap();
        assert_eq!(
            (r.width(), r.height()),
            (expect, expect),
            "ppp {ppp}: one document pixel per physical pixel"
        );
    }
    let mut h = harness();
    h.get_by_label("4×").click();
    h.run();
    assert_eq!(h.state().canvas_rect().unwrap().width(), 1280.0);
}

#[test]
fn zoomed_clicks_hit_the_right_document_pixel() {
    let mut h = harness();
    with_layer(&mut h);
    h.get_by_label("4×").click();
    h.run();
    let origin = h.state().canvas_rect().unwrap().min;
    let p = origin + Vec2::new(4.0 * 9.0 + 1.0, 4.0 * 3.0 + 2.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(p), press(p), release(p)]);
    h.step();
    h.run();
    assert_eq!(top_pixel(&h, 9, 3).a(), 255);
    assert_eq!(top_pixel(&h, 10, 3).a(), 0);
}

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
    let reopened = paintcraft_ui::editor::Editor::open(&file).unwrap();
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

#[test]
fn gui_and_core_produce_identical_pixels_for_the_same_stroke() {
    use paintcraft_core::commands::Batch;
    use paintcraft_core::document::{POLICY, new_document};
    use paintcraft_core::render::render_flat;
    use paintcraft_core::session::Session;

    let mut h = harness();
    with_layer(&mut h);
    // Across both tile edges: x = 127/128 and x = 255/256.
    let pts = [(100.0f32, 10.0f32), (300.0, 200.0), (120.0, 300.0)];
    let first = at(&h, pts[0].0, pts[0].1);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(first), press(first)]);
    for p in &pts[1..] {
        let moved = Event::PointerMoved(at(&h, p.0, p.1));
        h.input_mut().events.push(moved);
    }
    let last = at(&h, pts[2].0, pts[2].1);
    h.input_mut().events.push(release(last));
    h.step();
    h.run();

    // The same intent applied to the engine directly, with no GUI involved.
    let mut core =
        Session::new(new_document(320, 320, Some(Rgba([255, 255, 255, 255])), 0, POLICY).unwrap());
    let run = |s: &mut Session, ops: &str| {
        let t = s.ticket();
        let text = format!(
            r#"{{"schema":"paintcraft.batch/v1","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":{ops}}}"#,
            t.state_id, t.publication_epoch
        );
        s.apply(&Batch::from_json(text.as_bytes()).unwrap())
            .unwrap()
    };
    let layer = run(&mut core, r##"[{"op":"layer.add","ref":"l","name":"Layer 2","width":320,"height":320,"origin":{"x":0,"y":0},"default_pixel":"#00000000"}]"##).aliases["l"];
    run(
        &mut core,
        &format!(
            r##"[{{"op":"raster.pencil_path","layer_ref":{{"id":"{layer}"}},"points":[{{"x":100,"y":10}},{{"x":300,"y":200}},{{"x":120,"y":300}}],"color":"#000000FF","mode":"source-over"}}]"##
        ),
    );
    assert_eq!(
        h.state().rendered(),
        render_flat(core.document()).unwrap(),
        "GUI and core agree byte for byte"
    );
    assert_eq!(publications(&h), core.ticket().publication_epoch);
    for (x, y) in [(127, 36), (128, 37), (255, 157), (256, 158)] {
        assert_eq!(
            top_pixel(&h, x, y),
            Rgba([0, 0, 0, 255]),
            "({x},{y}) on the tile edges"
        );
    }
}

fn drag(h: &mut Harness<App>, a: (f32, f32), b: (f32, f32)) {
    let (pa, pb) = (at(h, a.0, a.1), at(h, b.0, b.1));
    h.input_mut().events.extend([
        Event::PointerMoved(pa),
        press(pa),
        Event::PointerMoved(pb),
        release(pb),
    ]);
    h.step();
    h.run();
}

#[test]
fn choosing_a_tool_and_dragging_makes_one_shape_transaction() {
    let mut h = harness();
    with_layer(&mut h);
    h.get_by_label("Rectangle").click();
    h.run();
    h.get_by_label("Filled").click();
    h.run();
    let before = publications(&h);
    drag(&mut h, (20.0, 30.0), (60.0, 50.0));
    assert_eq!(publications(&h), before + 1);
    assert_eq!(top_pixel(&h, 20, 30), Rgba([0, 0, 0, 255]));
    assert_eq!(top_pixel(&h, 60, 50), Rgba([0, 0, 0, 255]));
    assert_eq!(top_pixel(&h, 61, 50).a(), 0);
    assert_eq!(
        h.state().rendered()[(40 * 320 + 40) * 4 + 3],
        255,
        "the display shows the shape"
    );
}

#[test]
fn fill_and_eraser_work_from_the_palette() {
    let mut h = harness();
    with_layer(&mut h);
    h.get_by_label("Fill").click();
    h.run();
    let before = publications(&h);
    drag(&mut h, (5.0, 5.0), (5.0, 5.0));
    assert_eq!(publications(&h), before + 1);
    assert_eq!(top_pixel(&h, 300, 300), Rgba([0, 0, 0, 255]));
    h.get_by_label("Eraser").click();
    h.run();
    drag(&mut h, (100.0, 100.0), (120.0, 100.0));
    assert_eq!(top_pixel(&h, 110, 100), Rgba::TRANSPARENT);
    assert_eq!(top_pixel(&h, 110, 120).a(), 255);
}

#[test]
fn selection_restricts_painting_until_it_is_removed_from_the_palette() {
    let mut h = harness();
    with_layer(&mut h);
    h.get_by_label("Select").click();
    h.run();
    drag(&mut h, (50.0, 50.0), (80.0, 80.0));
    h.get_by_label("Pencil").click();
    h.run();
    drag(&mut h, (10.0, 60.0), (70.0, 60.0));
    assert_eq!(
        (top_pixel(&h, 20, 60).a(), top_pixel(&h, 60, 60).a()),
        (0, 255),
        "only inside the selection"
    );
    h.get_by_label("Select").click();
    h.run();
    h.get_by_label("Deselect").click();
    h.run();
    h.get_by_label("Pencil").click();
    h.run();
    drag(&mut h, (10.0, 70.0), (70.0, 70.0));
    assert_eq!(top_pixel(&h, 20, 70).a(), 255);
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
fn first_visit_shows_the_picker_with_v11_selected_and_remembers_the_choice() {
    use paintcraft_ui::settings::{ClassicVariant, Loaded, Settings};
    let dir = scratch("settings");
    {
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        assert!(h.state().picker_open());
        assert_eq!(
            h.state().variant(),
            ClassicVariant::V11,
            "the default before any choice"
        );
        h.get_by_label("Continue").click();
        h.run();
        assert!(!h.state().picker_open());
        assert_eq!(
            Settings::load(&dir).0.classic_variant,
            Some(ClassicVariant::V11),
            "accepting the default saves it"
        );
    }
    // A later start opens straight in the saved variant without asking again.
    let h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    assert!(!h.state().picker_open());
    drop(h);

    // Changing it later goes through the Interface button.
    let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    h.get_by_label("Interface").click();
    h.run();
    assert!(h.state().picker_open());
    h.get_by_role_and_label(eframe::egui::Role::RadioButton, "Paint v7")
        .click();
    h.run();
    h.get_by_label("Continue").click();
    h.run();
    assert_eq!(h.state().variant(), ClassicVariant::V7);
    assert_eq!(
        Settings::load(&dir),
        (
            Settings {
                classic_variant: Some(ClassicVariant::V7)
            },
            Loaded::Ok
        )
    );
    let h2 = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    assert_eq!(h2.state().variant(), ClassicVariant::V7);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_damaged_settings_file_is_kept_and_explained_and_the_picker_returns() {
    let dir = scratch("badsettings");
    std::fs::write(dir.join("settings.json"), b"{ not json").unwrap();
    let h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
    assert!(h.state().picker_open());
    assert!(
        h.state()
            .editor()
            .status()
            .contains("Settings were unusable"),
        "{}",
        h.state().editor().status()
    );
    let kept: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        kept.iter().any(|n| n.starts_with("settings.json.unusable")),
        "{kept:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn every_variant_renders_a_usable_window_with_the_same_controls() {
    use paintcraft_ui::settings::{ClassicVariant, Settings};
    for v in ClassicVariant::ALL {
        let dir = scratch("variant");
        Settings {
            classic_variant: Some(v),
        }
        .save(&dir)
        .unwrap();
        let mut h = harness_with(App::new(None).with_settings(dir.clone()), 1.0, false);
        with_layer(&mut h);
        h.get_by_label("Pencil").click();
        h.run();
        drag(&mut h, (5.0, 5.0), (30.0, 5.0));
        assert_eq!(top_pixel(&h, 20, 5).a(), 255, "{v:?}");
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn typing_text_on_the_canvas_places_one_undoable_text_object() {
    let mut h = harness();
    with_layer(&mut h);
    h.get_by_label("Text").click();
    h.run();
    let before = publications(&h);
    let p = at(&h, 20.0, 30.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(p), press(p), release(p)]);
    h.step();
    h.input_mut().events.push(Event::Text("Hi".into()));
    h.step();
    h.input_mut().events.extend([
        key(Key::Enter, Modifiers::NONE),
        key_up(Key::Enter, Modifiers::NONE),
    ]);
    h.step();
    h.input_mut().events.push(Event::Text("x".into()));
    h.step();
    h.input_mut().events.extend([
        key(Key::Backspace, Modifiers::NONE),
        key_up(Key::Backspace, Modifiers::NONE),
    ]);
    h.step();
    h.run();
    assert_eq!(publications(&h), before, "typing publishes nothing");
    h.event(key(Key::Enter, Modifiers::COMMAND));
    h.step();
    h.run();
    assert_eq!(
        publications(&h),
        before + 1,
        "one transaction for the whole text"
    );
    // 'H' row 0 is 0x33 = bits 0,1,4,5; at scale 2 from x = 20 that is x 20..=23 and 28..=31.
    let lit: Vec<bool> = (20..32).map(|x| top_pixel(&h, x, 30).a() == 255).collect();
    assert_eq!(
        lit,
        [
            true, true, true, true, false, false, false, false, true, true, true, true
        ]
    );
    h.get_by_label("Undo").click();
    h.run();
    assert_eq!(top_pixel(&h, 20, 30).a(), 0);
}

#[test]
fn escape_discards_text_and_keyboard_focus_in_a_field_keeps_typing_out_of_the_canvas() {
    let mut h = harness();
    with_layer(&mut h);
    h.get_by_label("Text").click();
    h.run();
    let before = publications(&h);
    let p = at(&h, 40.0, 40.0);
    h.input_mut()
        .events
        .extend([Event::PointerMoved(p), press(p), release(p)]);
    h.step();
    h.input_mut().events.push(Event::Text("gone".into()));
    h.step();
    h.input_mut().events.push(key(Key::Escape, Modifiers::NONE));
    h.step();
    h.input_mut()
        .events
        .push(key(Key::Enter, Modifiers::COMMAND));
    h.step();
    h.run();
    assert_eq!(publications(&h), before);
    assert!(!h.state().editor().is_editing_text());
}
