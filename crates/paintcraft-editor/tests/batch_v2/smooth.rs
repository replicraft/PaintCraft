use super::*;

#[test]
fn antialiased_strokes_clip_and_apply_alpha_once() {
    let (mut s, id) = session(12, 12);
    run(
        &mut s,
        2,
        r#"[{"op":"selection.set_rect","rect":{"x":2,"y":2,"width":5,"height":5}}]"#,
    )
    .unwrap();
    let before = s.document().clone();
    run(
        &mut s,
        2,
        &format!(
            "[{}]",
            stroke(
                id,
                r#"[{"x":0,"y":0},{"x":10,"y":10},{"x":0,"y":0}]"#,
                "#20408080",
                5,
                "smooth-round"
            )
        ),
    )
    .unwrap();
    let raster = &s.document().layers[&id].raster;
    assert_eq!(raster.get(4, 4), Rgba([32, 64, 128, 128]));
    let mut partial = false;
    for y in 0..12 {
        for x in 0..12 {
            let a = raster.get(x, y).a();
            if !(2..7).contains(&x) || !(2..7).contains(&y) {
                assert_eq!(a, 0);
            }
            assert!(a <= 128, "retracing must not darken a stroke");
            partial |= a > 0 && a < 128;
        }
    }
    assert!(partial);
    s.undo().unwrap();
    assert!(s.document().same_semantics(&before));
}

#[test]
fn replace_interpolates_partial_coverage_and_zero_alpha_can_erase() {
    let (mut s, id) = session(9, 9);
    run(
        &mut s,
        2,
        &format!(
            "[{}]",
            stroke(id, r#"[{"x":4,"y":4}]"#, "#204080FF", 5, "smooth-round")
        ),
    )
    .unwrap();
    let before = s.document().layers[&id].raster.get(2, 2);
    assert!(before.a() > 0 && before.a() < 255);
    let erase = stroke(id, r#"[{"x":4,"y":4}]"#, "#00000000", 5, "smooth-round")
        .replace("source-over", "replace");
    run(&mut s, 2, &format!("[{erase}]")).unwrap();
    let raster = &s.document().layers[&id].raster;
    assert_eq!(raster.get(4, 4), Rgba::TRANSPARENT);
    assert!(raster.get(2, 2).a() > 0 && raster.get(2, 2).a() < before.a());
}

#[test]
fn smooth_work_limits_reject_the_entire_batch() {
    let (mut s, id) = session(16, 16);
    let ticket = s.ticket();
    let ops = format!(
        "[{},{}]",
        stroke(id, r#"[{"x":3,"y":3}]"#, "#000000FF", 3, "smooth-round"),
        stroke(
            id,
            r#"[{"x":0,"y":0},{"x":1000000,"y":1000000}]"#,
            "#000000FF",
            1024,
            "smooth-round"
        )
    );
    assert_eq!(
        run(&mut s, 2, &ops).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.document().layers[&id].raster.get(3, 3), Rgba::TRANSPARENT);
}
