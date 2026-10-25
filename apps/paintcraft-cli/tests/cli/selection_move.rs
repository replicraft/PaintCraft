use super::*;
use paintcraft_core::types::{Rect, Rgba};
use paintcraft_document::{Document, Layer, POLICY, import_image, new_document};

fn fixture(origin: (i64, i64)) -> Document {
    let rgba: Vec<_> = (0..4u8)
        .flat_map(|y| {
            (0..5u8).flat_map(move |x| {
                if (x, y) == (1, 1) {
                    [17, 33, 55, 0]
                } else if (x, y) == (2, 1) {
                    [17, 33, 55, 255]
                } else {
                    [x * 39, y * 51, 91, if (x + y) % 2 == 0 { 0 } else { 128 }]
                }
            })
        })
        .collect();
    let image = import_image(5, 4, &rgba, &format!("cli-move-{origin:?}")).unwrap();
    let mut target = image.layers[&image.roots[0]].clone();
    target.origin = origin;
    target.name = "Hidden moving layer 日本語".into();
    target.visible = false;
    target.opacity = 73;
    let mut doc = new_document(8, 6, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    doc.state_id = image.state_id;
    doc.title = Some("Floating selection CLI fixture".into());
    doc.roots.push(target.id);
    doc.layers.insert(target.id, target);
    doc
}
fn selected(masked: bool, x: i64, y: i64) -> bool {
    (3..7).contains(&x) && (2..5).contains(&y) && (!masked || !((4..6).contains(&x) && y == 3))
}
fn selection(masked: bool) -> String {
    if masked {
        // Outer 4x3 region and an inner 2x1 hole, joined by a doubled bridge.
        r#"{"op":"selection.set_polygon","points":[{"x":3,"y":2},{"x":7,"y":2},{"x":7,"y":5},{"x":3,"y":5},{"x":3,"y":2},{"x":4,"y":3},{"x":4,"y":4},{"x":6,"y":4},{"x":6,"y":3},{"x":4,"y":3},{"x":3,"y":2}]}"#.into()
    } else {
        r#"{"op":"selection.set_rect","rect":{"x":3,"y":2,"width":4,"height":3}}"#.into()
    }
}
fn moving(layer: &str, offset: (i64, i64), clear: Rgba, key: Option<Rgba>) -> String {
    let key = key
        .map(|color| format!(r##", "transparent_key":"{}""##, color.to_hex()))
        .unwrap_or_default();
    format!(
        r##"{{"op":"raster.move_selection","layer_ref":{{"id":"{layer}"}},"offset":{{"x":{},"y":{}}},"clear":"{}"{key}}}"##,
        offset.0,
        offset.1,
        clear.to_hex()
    )
}
fn batch(state: &str, version: u8, operations: &[String]) -> String {
    format!(
        r#"{{"schema":"paintcraft.batch/v{version}","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{state}","publication_epoch":"0"}},"operations":[{}]}}"#,
        operations.join(",")
    )
}
fn prefix(layer: &str) -> String {
    // Change a selected source to the transparent key before extraction. A key
    // classifier reading the original raster would paste the wrong old pixel.
    format!(
        r##"{{"op":"raster.fill_rect","layer_ref":{{"id":"{layer}"}},"rect":{{"x":4,"y":2,"width":1,"height":1}},"color":"#11213700","mode":"replace"}}"##
    )
}
fn pixels(layer: &Layer) -> Vec<Vec<Rgba>> {
    (0..layer.raster.height())
        .map(|y| {
            (0..layer.raster.width())
                .map(|x| layer.raster.get(x.into(), y.into()))
                .collect()
        })
        .collect()
}
fn oracle(
    layer: &Layer,
    masked: bool,
    offset: (i64, i64),
    clear: Rgba,
    key: Option<Rgba>,
    paint: bool,
) -> ((i64, i64), Vec<Vec<Rgba>>) {
    let mut source = pixels(layer);
    let old = layer.extent();
    if paint && old.contains(4, 2) {
        source[(2 - layer.origin.1) as usize][(4 - layer.origin.0) as usize] =
            Rgba([17, 33, 55, 0]);
    }
    if offset == (0, 0) {
        return (layer.origin, source);
    }
    let at = |x: i64, y: i64| {
        if old.contains(x, y) {
            source[(y - layer.origin.1) as usize][(x - layer.origin.0) as usize]
        } else {
            Rgba::TRANSPARENT
        }
    };
    let destination = Rect::new(3 + offset.0, 2 + offset.1, 4, 3).unwrap();
    let (ox, oy) = (old.x0.min(destination.x0), old.y0.min(destination.y0));
    let width = old.x1.max(destination.x1) - ox;
    let height = old.y1.max(destination.y1) - oy;
    let mut out = vec![vec![Rgba::TRANSPARENT; width as usize]; height as usize];
    for y in old.y0..old.y1 {
        for x in old.x0..old.x1 {
            out[(y - oy) as usize][(x - ox) as usize] = if selected(masked, x, y) {
                clear
            } else {
                at(x, y)
            };
        }
    }
    for y in 2..5 {
        for x in 3..7 {
            if !selected(masked, x, y) {
                continue;
            }
            let color = at(x, y);
            if key.is_some_and(|key| key.0[..3] == color.0[..3]) {
                continue;
            }
            out[(y + offset.1 - oy) as usize][(x + offset.0 - ox) as usize] = color;
        }
    }
    ((ox, oy), out)
}
#[test]
fn real_cli_moves_rectangles_and_holes_with_rgb_keys_logical_prefixes_and_off_canvas_storage() {
    let scratch = Scratch::new("selection-move-matrix");
    let operations = scratch.path("operations.json");
    let mut case = 0;
    for origin in [(-2, -1), (3, 2), (7, 4)] {
        let doc = fixture(origin);
        let id = doc.roots[1];
        let layer = id.to_string();
        let base = scratch.path(&format!("base-{case}.pcnd"));
        let original = paintcraft_formats::pcnd::write(&doc).unwrap();
        fs::write(&base, &original).unwrap();
        for masked in [false, true] {
            for offset in [(2, 1), (-4, -3), (0, 0)] {
                for key in [
                    None,
                    Some(Rgba([17, 33, 55, 0])),
                    Some(Rgba([17, 33, 55, 255])),
                ] {
                    for clear in [Rgba::TRANSPARENT, Rgba([255; 4])] {
                        for paint in [false, true] {
                            let destination = scratch.path(&format!("move-{case}.pcnd"));
                            case += 1;
                            let mut ops = Vec::new();
                            if paint {
                                ops.push(prefix(&layer));
                            }
                            ops.push(selection(masked));
                            ops.push(moving(&layer, offset, clear, key));
                            fs::write(&operations, batch(&doc.state_id.to_string(), 2, &ops))
                                .unwrap();
                            let applied = run(&[
                                "apply",
                                &base,
                                "--operations",
                                &operations,
                                "-o",
                                &destination,
                                "--json",
                            ]);
                            assert_eq!(
                                applied.code, 0,
                                "case{case} {} {}",
                                applied.stdout, applied.stderr
                            );
                            let reopened =
                                paintcraft_formats::pcnd::read(&fs::read(&destination).unwrap())
                                    .unwrap();
                            let expected =
                                oracle(&doc.layers[&id], masked, offset, clear, key, paint);
                            assert_eq!(reopened.canvas, doc.canvas);
                            assert_eq!(reopened.document_id, doc.document_id);
                            assert_eq!(reopened.roots, doc.roots);
                            assert_eq!(reopened.title, doc.title);
                            assert_eq!(
                                reopened.layers[&doc.roots[0]], doc.layers[&doc.roots[0]],
                                "unselected layer changed"
                            );
                            let actual = &reopened.layers[&id];
                            let before = &doc.layers[&id];
                            assert_eq!(
                                (&actual.name, actual.visible, actual.opacity),
                                (&before.name, before.visible, before.opacity)
                            );
                            assert_eq!(actual.origin, expected.0, "case{case}");
                            assert_eq!(
                                pixels(actual),
                                expected.1,
                                "case{case} mask={masked} offset={offset:?} key={key:?} clear={clear:?} prefix={paint}"
                            );
                            assert_eq!(fs::read(&base).unwrap(), original, "CLI mutated its input");
                        }
                    }
                }
            }
        }
    }
    assert_eq!(case, 216);
}
#[test]
fn zero_move_with_key_preserves_canonical_archive_and_state_and_reports_uncommitted() {
    let scratch = Scratch::new("selection-move-zero");
    let doc = fixture((3, 2));
    let id = doc.roots[1].to_string();
    let base = scratch.path("base.pcnd");
    let original = paintcraft_formats::pcnd::write(&doc).unwrap();
    fs::write(&base, &original).unwrap();
    let operations = scratch.path("operations.json");
    for masked in [false, true] {
        let destination = scratch.path(if masked { "mask.pcnd" } else { "rect.pcnd" });
        // Selection is session-only. Restore its initial inactive value to isolate
        // the no-op movement from an otherwise real selection-only publication.
        let ops = vec![
            selection(masked),
            moving(&id, (0, 0), Rgba([255; 4]), Some(Rgba([17, 33, 55, 255]))),
            r#"{"op":"selection.clear"}"#.into(),
        ];
        fs::write(&operations, batch(&doc.state_id.to_string(), 2, &ops)).unwrap();
        let applied = run(&[
            "apply",
            &base,
            "--operations",
            &operations,
            "-o",
            &destination,
            "--json",
        ]);
        assert_eq!(applied.code, 0, "{} {}", applied.stdout, applied.stderr);
        let receipt = applied.json();
        assert_eq!(
            receipt.get("engine").unwrap().get("committed"),
            Some(&Value::Bool(false))
        );
        assert_eq!(
            text(&receipt, &["engine", "state_id"]),
            doc.state_id.to_string()
        );
        assert_eq!(text(&receipt, &["engine", "publication_epoch"]), "0");
        let saved = fs::read(&destination).unwrap();
        assert_eq!(
            saved,
            paintcraft_formats::pcnd::write_compact(&doc).unwrap()
        );
        let restored = paintcraft_formats::pcnd::read(&saved).unwrap();
        assert_eq!(
            paintcraft_formats::pcnd::write(&restored).unwrap(),
            original
        );
        assert_eq!(
            paintcraft_formats::pcnd::read(&fs::read(destination).unwrap())
                .unwrap()
                .state_id,
            doc.state_id
        );
    }
    assert_eq!(fs::read(base).unwrap(), original);
}
#[test]
fn failed_late_move_batch_preserves_sentinel_and_input_with_correct_operation_index() {
    let scratch = Scratch::new("selection-move-rollback");
    let doc = fixture((3, 2));
    let id = doc.roots[1].to_string();
    let base = scratch.path("base.pcnd");
    let output = scratch.path("sentinel.pcnd");
    let operations = scratch.path("operations.json");
    let original = paintcraft_formats::pcnd::write(&doc).unwrap();
    fs::write(&base, &original).unwrap();
    fs::write(&output, b"untouched destination sentinel").unwrap();
    let ops = vec![
        prefix(&id),
        selection(true),
        moving(&id, (2, 1), Rgba([255; 4]), Some(Rgba([17, 33, 55, 0]))),
        r#"{"op":"canvas.crop","rect":{"x":8,"y":0,"width":1,"height":1}}"#.into(),
    ];
    fs::write(&operations, batch(&doc.state_id.to_string(), 2, &ops)).unwrap();
    for destination in [&output, &base] {
        let refused = run(&[
            "apply",
            &base,
            "--operations",
            &operations,
            "-o",
            destination,
            "--overwrite",
            "--json",
        ]);
        assert_eq!(refused.code, 2, "{} {}", refused.stdout, refused.stderr);
        let error = refused.json();
        assert_eq!(text(&error, &["engine", "code"]), "input.schema");
        assert_eq!(
            error.get("engine").unwrap().get("operation_index"),
            Some(&Value::Int(3))
        );
        assert_eq!(
            error.get("engine").unwrap().get("committed"),
            Some(&Value::Bool(false))
        );
        assert_eq!(fs::read(&base).unwrap(), original);
        assert_eq!(
            fs::read(&output).unwrap(),
            b"untouched destination sentinel"
        );
    }
}
#[test]
fn cli_rejects_v1_move_and_exports_it_only_in_v2_schema() {
    let scratch = Scratch::new("selection-move-v1");
    let doc = fixture((3, 2));
    let id = doc.roots[1].to_string();
    let base = scratch.path("base.pcnd");
    let operations = scratch.path("operations.json");
    let destination = scratch.path("must-not-exist.pcnd");
    let original = paintcraft_formats::pcnd::write(&doc).unwrap();
    fs::write(&base, &original).unwrap();
    fs::write(
        &operations,
        batch(
            &doc.state_id.to_string(),
            1,
            &[selection(false), moving(&id, (1, 1), Rgba([255; 4]), None)],
        ),
    )
    .unwrap();
    let refused = run(&[
        "apply",
        &base,
        "--operations",
        &operations,
        "-o",
        &destination,
        "--json",
    ]);
    assert_eq!(refused.code, 2, "{} {}", refused.stdout, refused.stderr);
    let error = refused.json();
    assert_eq!(text(&error, &["engine", "code"]), "input.schema");
    // Schema rejection happens before execution; this existing wire contract
    // omits an operation index, unlike the late engine failure tested above.
    assert_eq!(error.get("engine").unwrap().get("operation_index"), None);
    assert!(!Path::new(&destination).exists());
    assert_eq!(fs::read(base).unwrap(), original);
    let v1 = run(&["schema", "batch"]);
    let v2 = run(&["schema", "batch-v2"]);
    assert_eq!((v1.code, v2.code), (0, 0));
    assert!(!v1.stdout.contains("raster.move_selection"));
    assert!(v2.stdout.contains("raster.move_selection"));
}
