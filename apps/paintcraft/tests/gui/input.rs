use super::*;

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
fn gui_and_core_produce_identical_pixels_for_the_same_stroke() {
    use paintcraft_commands::Batch;
    use paintcraft_document::{POLICY, new_document};
    use paintcraft_editor::session::Session;
    use paintcraft_render::render_flat;

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

#[test]
fn classic_tool_actions_survive_multiple_layout_passes_without_repeating_a_shape() {
    let mut h = harness_with(App::new(None), 1.0, true);
    with_layer(&mut h);
    h.get_by_label("Rectangle").click();
    h.run();
    h.get_by_label("Filled").click();
    h.run();
    let before = publications(&h);
    drag(&mut h, (5.0, 5.0), (25.0, 25.0));
    assert_eq!(publications(&h), before + 1);
    assert_eq!(top_pixel(&h, 15, 15).a(), 255);
}
