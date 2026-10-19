//! Batch v2 operations: brush strokes, flood fill and shapes. Expected pictures
//! are derived by hand from the geometry definitions, not copied from output.

use paintcraft_core::document::{POLICY, new_document};
use paintcraft_core::session::{Receipt, Session};
use paintcraft_types::{Error, ErrorCode, Rgba};
use uuid::Uuid;

fn session(w: u32, h: u32) -> (Session, Uuid) {
    let mut s = Session::new(new_document(w, h, None, 0, POLICY).unwrap());
    let r = run(&mut s, 2, &format!(r##"[{{"op":"layer.add","ref":"l","name":"L","width":{w},"height":{h},"origin":{{"x":0,"y":0}},"default_pixel":"#00000000"}}]"##)).unwrap();
    let id = r.aliases["l"];
    (s, id)
}

fn run(s: &mut Session, version: u8, ops: &str) -> Result<Receipt, Error> {
    let t = s.ticket();
    let text = format!(
        r#"{{"schema":"paintcraft.batch/v{version}","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":{ops}}}"#,
        t.state_id, t.publication_epoch
    );
    s.apply_json(text.as_bytes())
}

/// ASCII picture of a layer: `#` opaque, `+` translucent, `.` empty.
fn picture(s: &Session, id: Uuid) -> Vec<String> {
    let l = &s.document().layers[&id].raster;
    (0..i64::from(l.height()))
        .map(|y| {
            (0..i64::from(l.width()))
                .map(|x| match l.get(x, y).a() {
                    0 => '.',
                    255 => '#',
                    _ => '+',
                })
                .collect()
        })
        .collect()
}

fn lref(id: Uuid) -> String {
    format!(r#"{{"id":"{id}"}}"#)
}

fn stroke(id: Uuid, pts: &str, color: &str, size: u32, brush: &str) -> String {
    format!(
        r#"{{"op":"raster.stroke_path","layer_ref":{},"points":{pts},"color":"{color}","mode":"source-over","size":{size},"brush":"{brush}"}}"#,
        lref(id)
    )
}

#[test]
fn v1_rejects_v2_operations_and_v2_accepts_v1_ones() {
    let (mut s, id) = session(4, 4);
    let e = run(
        &mut s,
        1,
        &format!(
            "[{}]",
            stroke(id, r#"[{"x":0,"y":0}]"#, "#000000FF", 1, "square")
        ),
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::InputSchema);
    assert!(e.message.contains("batch/v2"));
    let ok = run(&mut s, 2, &format!(r##"[{{"op":"raster.fill_rect","layer_ref":{},"rect":{{"x":0,"y":0,"width":1,"height":1}},"color":"#000000FF","mode":"replace"}}]"##, lref(id))).unwrap();
    assert!(ok.committed);
    for bad in [
        stroke(id, r#"[{"x":0,"y":0}]"#, "#000000FF", 0, "square"),
        stroke(id, r#"[{"x":0,"y":0}]"#, "#000000FF", 1025, "square"),
        stroke(id, r#"[{"x":0,"y":0}]"#, "#000000FF", 1, "triangle"),
        format!(
            r#"{{"op":"raster.shape","layer_ref":{},"kind":"rect","from":{{"x":0,"y":0}},"to":{{"x":1,"y":1}},"mode":"replace"}}"#,
            lref(id)
        ),
        format!(
            r##"{{"op":"raster.shape","layer_ref":{},"kind":"line","from":{{"x":0,"y":0}},"to":{{"x":1,"y":1}},"fill":"#000000FF","mode":"replace"}}"##,
            lref(id)
        ),
        format!(
            r##"{{"op":"raster.flood_fill","layer_ref":{},"at":{{"x":0,"y":0}},"color":"#000000FF","mode":"replace","tolerance":256}}"##,
            lref(id)
        ),
    ] {
        assert_eq!(
            run(&mut s, 2, &format!("[{bad}]")).unwrap_err().code,
            ErrorCode::InputSchema,
            "{bad}"
        );
    }
}

#[test]
fn square_and_round_strokes_have_the_documented_footprint() {
    let (mut s, id) = session(10, 7);
    run(
        &mut s,
        2,
        &format!(
            "[{}]",
            stroke(
                id,
                r#"[{"x":2,"y":2},{"x":6,"y":2}]"#,
                "#000000FF",
                3,
                "square"
            )
        ),
    )
    .unwrap();
    assert_eq!(
        picture(&s, id)[..5],
        [
            "..........",
            ".#######..",
            ".#######..",
            ".#######..",
            ".........."
        ]
        .map(String::from)
    );
    let (mut s, id) = session(9, 9);
    run(
        &mut s,
        2,
        &format!(
            "[{}]",
            stroke(id, r#"[{"x":4,"y":4}]"#, "#000000FF", 5, "round")
        ),
    )
    .unwrap();
    assert_eq!(
        picture(&s, id)[1..8],
        [
            ".........",
            "...###...",
            "..#####..",
            "..#####..",
            "..#####..",
            "...###...",
            "........."
        ]
        .map(String::from)
    );
}

#[test]
fn translucent_strokes_blend_each_pixel_once_however_they_overlap() {
    let (mut s, id) = session(12, 3);
    // Overlapping stamps and a path that doubles back over itself.
    run(
        &mut s,
        2,
        &format!(
            "[{}]",
            stroke(
                id,
                r#"[{"x":2,"y":1},{"x":9,"y":1},{"x":3,"y":1}]"#,
                "#FF000080",
                3,
                "square"
            )
        ),
    )
    .unwrap();
    let l = &s.document().layers[&id].raster;
    for x in 1..=10 {
        assert_eq!(
            l.get(x, 1),
            Rgba([255, 0, 0, 128]),
            "x = {x}: alpha 128 exactly, not accumulated"
        );
    }
    assert_eq!(l.get(0, 1), Rgba::TRANSPARENT);
}

#[test]
fn erasing_with_replace_transparent_and_selection_gating() {
    let (mut s, id) = session(6, 3);
    run(&mut s, 2, &format!(r##"[{{"op":"raster.fill_rect","layer_ref":{},"rect":{{"x":0,"y":0,"width":6,"height":3}},"color":"#112233FF","mode":"replace"}}]"##, lref(id))).unwrap();
    run(
        &mut s,
        2,
        r#"[{"op":"selection.set_rect","rect":{"x":0,"y":0,"width":3,"height":3}}]"#,
    )
    .unwrap();
    let erase = format!(
        r##"[{{"op":"raster.stroke_path","layer_ref":{},"points":[{{"x":0,"y":1}},{{"x":5,"y":1}}],"color":"#00000000","mode":"replace","size":1,"brush":"square"}}]"##,
        lref(id)
    );
    run(&mut s, 2, &erase).unwrap();
    assert_eq!(picture(&s, id)[1], "...###");
    assert_eq!(
        s.document().layers[&id].raster.get(0, 1),
        Rgba([0, 0, 0, 0])
    );
    // Erasing again changes nothing, so it is a semantic no-op.
    assert!(!run(&mut s, 2, &erase).unwrap().committed);
}

#[test]
fn flood_fill_respects_walls_selection_and_tolerance() {
    let (mut s, id) = session(7, 5);
    // A closed box with walls at x=1..5,y=0..4 border drawn as an outline.
    run(&mut s, 2, &format!(r##"[{{"op":"raster.shape","layer_ref":{},"kind":"rect","from":{{"x":1,"y":0}},"to":{{"x":5,"y":4}},"stroke":{{"color":"#000000FF","size":1}},"mode":"replace"}}]"##, lref(id))).unwrap();
    let fill = |at: &str, tol: u32| {
        format!(
            r##"[{{"op":"raster.flood_fill","layer_ref":{},"at":{at},"color":"#FF0000FF","mode":"replace","tolerance":{tol}}}]"##,
            lref(id)
        )
    };
    run(&mut s, 2, &fill(r#"{"x":3,"y":2}"#, 0)).unwrap();
    let l = &s.document().layers[&id].raster;
    assert_eq!(l.get(3, 2), Rgba([255, 0, 0, 255]));
    assert_eq!(l.get(2, 1), Rgba([255, 0, 0, 255]));
    assert_eq!(l.get(1, 1), Rgba([0, 0, 0, 255]), "wall untouched");
    assert_eq!(
        l.get(0, 2),
        Rgba::TRANSPARENT,
        "outside the walls untouched"
    );
    // Filling the red interior with the same colour changes nothing.
    assert!(
        !run(&mut s, 2, &fill(r#"{"x":3,"y":2}"#, 0))
            .unwrap()
            .committed
    );
    // Selection limits the region; a seed outside it fills nothing.
    run(
        &mut s,
        2,
        r#"[{"op":"selection.set_rect","rect":{"x":0,"y":0,"width":2,"height":5}}]"#,
    )
    .unwrap();
    assert!(
        !run(&mut s, 2, &fill(r#"{"x":6,"y":2}"#, 0))
            .unwrap()
            .committed
    );
    run(&mut s, 2, &fill(r#"{"x":0,"y":2}"#, 0)).unwrap();
    let l = &s.document().layers[&id].raster;
    assert_eq!(
        (l.get(0, 0), l.get(0, 4), l.get(2, 2)),
        (
            Rgba([255, 0, 0, 255]),
            Rgba([255, 0, 0, 255]),
            Rgba([255, 0, 0, 255])
        )
    );
    assert_eq!(l.get(6, 2), Rgba::TRANSPARENT, "beyond the selection");
    // Tolerance 255 turns the whole selection into one region and replaces it.
    run(&mut s, 2, &fill(r#"{"x":0,"y":0}"#, 255)).unwrap();
    assert_eq!(
        s.document().layers[&id].raster.get(1, 0),
        Rgba([255, 0, 0, 255])
    );
}

#[test]
fn shapes_fill_and_outline_exactly() {
    let (mut s, id) = session(8, 6);
    let shape = |kind: &str, from: &str, to: &str, extra: &str| {
        format!(
            r##"[{{"op":"raster.shape","layer_ref":{},"kind":"{kind}","from":{from},"to":{to},{extra}"mode":"replace"}}]"##,
            lref(id)
        )
    };
    run(
        &mut s,
        2,
        &shape(
            "rect",
            r#"{"x":1,"y":1}"#,
            r#"{"x":5,"y":4}"#,
            r##""stroke":{"color":"#000000FF","size":1},"fill":"#FF0000FF","##,
        ),
    )
    .unwrap();
    let l = &s.document().layers[&id].raster;
    assert_eq!(
        (l.get(1, 1), l.get(5, 4), l.get(3, 2), l.get(6, 2)),
        (
            Rgba([0, 0, 0, 255]),
            Rgba([0, 0, 0, 255]),
            Rgba([255, 0, 0, 255]),
            Rgba::TRANSPARENT
        )
    );
    // Corners may be given in any order.
    let (mut s2, id2) = session(8, 6);
    let flipped = format!(
        r##"[{{"op":"raster.shape","layer_ref":{},"kind":"rect","from":{{"x":5,"y":4}},"to":{{"x":1,"y":1}},"stroke":{{"color":"#000000FF","size":1}},"fill":"#FF0000FF","mode":"replace"}}]"##,
        lref(id2)
    );
    run(&mut s2, 2, &flipped).unwrap();
    assert_eq!(picture(&s2, id2), picture(&s, id));

    let (mut s, id) = session(8, 8);
    run(&mut s, 2, &format!(r##"[{{"op":"raster.shape","layer_ref":{},"kind":"ellipse","from":{{"x":1,"y":1}},"to":{{"x":5,"y":5}},"fill":"#000000FF","mode":"replace"}}]"##, lref(id))).unwrap();
    assert_eq!(
        picture(&s, id)[1..6],
        [".###....", "#####...", "#####...", "#####...", ".###...."]
            .map(|r| format!(".{}", &r[..7]))
    );
    // A line is a round stroke between the corners.
    let (mut s, id) = session(6, 3);
    run(&mut s, 2, &format!(r##"[{{"op":"raster.shape","layer_ref":{},"kind":"line","from":{{"x":0,"y":1}},"to":{{"x":5,"y":1}},"stroke":{{"color":"#000000FF","size":1}},"mode":"replace"}}]"##, lref(id))).unwrap();
    assert_eq!(picture(&s, id)[1], "######");
}

#[test]
fn work_budgets_bound_strokes_and_every_failure_is_atomic() {
    let (mut s, id) = session(4, 4);
    let ticket = s.ticket();
    // 1 + 16,000,000 path pixels at diameter 2 is 32M stamp rows: over the budget, before any drawing.
    let far = r#"[{"x":0,"y":0},{"x":16000000,"y":0}]"#;
    let e = run(
        &mut s,
        2,
        &format!("[{}]", stroke(id, far, "#000000FF", 2, "round")),
    )
    .unwrap_err();
    assert_eq!(
        (e.code, e.operation_index),
        (ErrorCode::ResourceLimit, Some(0))
    );
    assert_eq!(s.ticket(), ticket);
    // A huge shape or fill on a huge layer is bounded by staging admission or the flood area.
    let mut big = Session::new(new_document(2, 2, None, 0, POLICY).unwrap());
    let r = run(&mut big, 2, r##"[{"op":"layer.add","ref":"l","name":"L","width":1048576,"height":1048576,"origin":{"x":0,"y":0},"default_pixel":"#00000000"}]"##).unwrap();
    let bid = r.aliases["l"];
    let fill = format!(
        r##"[{{"op":"raster.flood_fill","layer_ref":{},"at":{{"x":0,"y":0}},"color":"#000000FF","mode":"replace","tolerance":0}}]"##,
        lref(bid)
    );
    // The canvas is 2x2, so the editable area is tiny and this simply works.
    assert!(run(&mut big, 2, &fill).unwrap().committed);
    assert_eq!(big.document().layers[&bid].raster.resident_tiles(), 1);
}

#[test]
fn undo_restores_the_exact_state_after_every_v2_operation() {
    let (mut s, id) = session(8, 8);
    let before = s.ticket().state_id;
    let ops = [
        stroke(
            id,
            r#"[{"x":1,"y":1},{"x":6,"y":6}]"#,
            "#00FF0080",
            3,
            "round",
        ),
        format!(
            r##"{{"op":"raster.shape","layer_ref":{},"kind":"ellipse","from":{{"x":0,"y":0}},"to":{{"x":7,"y":7}},"stroke":{{"color":"#0000FFFF","size":2}},"mode":"source-over"}}"##,
            lref(id)
        ),
        format!(
            r##"{{"op":"raster.flood_fill","layer_ref":{},"at":{{"x":4,"y":4}},"color":"#FFFF00FF","mode":"replace","tolerance":16}}"##,
            lref(id)
        ),
    ];
    for op in &ops {
        assert!(run(&mut s, 2, &format!("[{op}]")).unwrap().committed);
    }
    for _ in &ops {
        s.undo().unwrap();
    }
    assert_eq!(s.ticket().state_id, before);
    assert_eq!(picture(&s, id), vec!["........"; 8]);
}

#[test]
fn parser_agrees_with_the_v2_json_schema_on_every_grammar_case() {
    use paintcraft_core::commands::Batch;
    use paintcraft_types::json::{self, Value};
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/grammar-v2.cases");
    let doc = json::parse(&std::fs::read(path).unwrap()).unwrap();
    let cases = doc.get("cases").and_then(Value::as_array).unwrap();
    assert!(cases.len() >= 15);
    for (i, c) in cases.iter().enumerate() {
        let expect = c.get("valid").and_then(Value::as_bool).unwrap();
        let got = Batch::from_value(c.get("batch").unwrap());
        assert_eq!(got.is_ok(), expect, "case {i}: {:?}", got.err());
    }
}
