use super::*;

const POLYGON: &str =
    r#"{"op":"selection.set_polygon","points":[{"x":1,"y":1},{"x":7,"y":1},{"x":3,"y":5}]}"#;

fn batch(state: &str, operations: &str) -> String {
    format!(
        r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{state}","publication_epoch":"0"}},"operations":[{operations}]}}"#
    )
}

#[test]
fn selection_masks_are_session_only_and_unsupported_transform_is_atomic() {
    let scratch = Scratch::new("selection-masks");
    let base = scratch.path("base.pcnd");
    let operations = scratch.path("operations.json");
    let output = scratch.path("output.pcnd");
    let created = run(&[
        "new",
        "--width",
        "8",
        "--height",
        "6",
        "--background",
        "#FFFFFFFF",
        "-o",
        &base,
    ]);
    assert_eq!(created.code, 0, "{}", created.stderr);
    let original = fs::read(&base).unwrap();
    let doc = paintcraft_formats::pcnd::read(&original).unwrap();
    let state = doc.state_id.to_string();
    let layer = doc.roots[0];

    for (name, selection_ops) in [
        ("polygon", POLYGON.to_owned()),
        (
            "inverted",
            format!(r#"{POLYGON},{{"op":"selection.invert"}}"#),
        ),
    ] {
        let destination = scratch.path(&format!("{name}.pcnd"));
        fs::write(&operations, batch(&state, &selection_ops)).unwrap();
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
        let result = applied.json();
        assert_eq!(
            result.get("engine").unwrap().get("committed"),
            Some(&Value::Bool(true))
        );
        assert_eq!(text(&result, &["engine", "state_id"]), state);
        assert_eq!(text(&result, &["engine", "publication_epoch"]), "1");
        assert_eq!(
            fs::read(destination).unwrap(),
            original,
            "selection is not document data"
        );
    }

    let paint = format!(
        r##"{{"op":"raster.fill_rect","layer_ref":{{"id":"{layer}"}},"rect":{{"x":0,"y":0,"width":8,"height":6}},"color":"#13579BFF","mode":"replace"}}"##
    );
    // A real private paint prefix must also roll back when a later mask transform fails.
    fs::write(
        &operations,
        batch(
            &state,
            &format!(r#"{paint},{POLYGON},{{"op":"canvas.transform","transform":"rotate-right"}}"#),
        ),
    )
    .unwrap();
    fs::write(&output, b"existing output must survive").unwrap();
    let refused = run(&[
        "apply",
        &base,
        "--operations",
        &operations,
        "-o",
        &output,
        "--overwrite",
        "--json",
    ]);
    assert_eq!(refused.code, 2, "{} {}", refused.stdout, refused.stderr);
    let error = refused.json();
    assert_eq!(text(&error, &["engine", "code"]), "input.schema");
    assert_eq!(
        error.get("engine").unwrap().get("operation_index"),
        Some(&Value::Int(2))
    );
    assert_eq!(
        error.get("engine").unwrap().get("committed"),
        Some(&Value::Bool(false))
    );
    assert_eq!(fs::read(&base).unwrap(), original);
    assert_eq!(fs::read(&output).unwrap(), b"existing output must survive");

    fs::write(
        &operations,
        batch(
            &state,
            &format!(r#"{POLYGON},{{"op":"selection.invert"}},{{"op":"selection.clear"}},{paint}"#),
        ),
    )
    .unwrap();
    let applied = run(&[
        "apply",
        &base,
        "--operations",
        &operations,
        "-o",
        &output,
        "--overwrite",
        "--json",
    ]);
    assert_eq!(applied.code, 0, "{} {}", applied.stdout, applied.stderr);
    let reopened = paintcraft_formats::pcnd::read(&fs::read(&output).unwrap()).unwrap();
    assert_ne!(reopened.state_id, doc.state_id);
    assert_eq!(reopened.canvas, doc.canvas);
    for y in 0..6 {
        for x in 0..8 {
            assert_eq!(
                reopened.layers[&layer].raster.get(x, y).0,
                [19, 87, 155, 255]
            );
        }
    }
    assert_eq!(fs::read(&base).unwrap(), original);
}

#[test]
fn exported_v2_schema_describes_strict_polygon_and_inversion_without_changing_v1() {
    let v2 = run(&["schema", "batch-v2"]);
    assert_eq!(v2.code, 0);
    let schema = v2.json();
    let operations = schema.get("properties").unwrap().get("operations").unwrap();
    let variants = operations
        .get("items")
        .unwrap()
        .get("oneOf")
        .unwrap()
        .as_array()
        .unwrap();
    let variant = |name| {
        variants
            .iter()
            .find(|v| {
                v.get("properties")
                    .and_then(|p| p.get("op"))
                    .and_then(|op| op.get("const"))
                    == Some(&Value::str(name))
            })
            .unwrap()
    };
    let polygon = variant("selection.set_polygon");
    assert_eq!(
        polygon.get("additionalProperties"),
        Some(&Value::Bool(false))
    );
    let points = polygon.get("properties").unwrap().get("points").unwrap();
    assert_eq!(points.get("minItems"), Some(&Value::Int(3)));
    assert_eq!(points.get("maxItems"), Some(&Value::Int(65536)));
    let point = points.get("items").unwrap();
    assert_eq!(point.get("additionalProperties"), Some(&Value::Bool(false)));
    for axis in ["x", "y"] {
        let coordinate = point.get("properties").unwrap().get(axis).unwrap();
        assert_eq!(text(coordinate, &["type"]), "integer");
        assert_eq!(
            coordinate.get("minimum"),
            Some(&Value::Int(-140737488355327))
        );
        assert_eq!(
            coordinate.get("maximum"),
            Some(&Value::Int(140737488355327))
        );
    }
    let invert = variant("selection.invert");
    assert_eq!(
        invert.get("additionalProperties"),
        Some(&Value::Bool(false))
    );
    assert_eq!(
        invert.get("required").unwrap().as_array().unwrap(),
        &[Value::str("op")]
    );
    let v1 = run(&["schema", "batch"]);
    assert_eq!(v1.code, 0);
    assert!(!v1.stdout.contains("selection.set_polygon"));
    assert!(!v1.stdout.contains("selection.invert"));
}

#[test]
fn polygon_paint_then_finite_inversion_paints_exact_complement_after_pcnd_reopen() {
    let scratch = Scratch::new("masked-paint");
    let base = scratch.path("base.pcnd");
    let operations = scratch.path("operations.json");
    assert_eq!(
        run(&[
            "new",
            "--width",
            "8",
            "--height",
            "6",
            "--background",
            "#FFFFFFFF",
            "-o",
            &base
        ])
        .code,
        0
    );
    let doc = paintcraft_formats::pcnd::read(&fs::read(&base).unwrap()).unwrap();
    let layer = doc.roots[0];
    let paint = |color| {
        format!(
            r##"{{"op":"raster.fill_rect","layer_ref":{{"id":"{layer}"}},"rect":{{"x":0,"y":0,"width":8,"height":6}},"color":"{color}","mode":"replace"}}"##
        )
    };
    // Independently specified pixel-center triangle rows, including excluded right edges.
    let triangle = [
        "........", ".#####..", "..###...", "..##....", "........", "........",
    ];
    for complementary in [false, true] {
        let destination = scratch.path(if complementary {
            "complement.pcnd"
        } else {
            "triangle.pcnd"
        });
        let mut ops = format!("{POLYGON},{}", paint("#D02040FF"));
        if complementary {
            ops.push_str(&format!(
                r#",{{"op":"selection.invert"}},{}"#,
                paint("#3060A0FF")
            ));
        }
        fs::write(&operations, batch(&doc.state_id.to_string(), &ops)).unwrap();
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
        let reopened = paintcraft_formats::pcnd::read(&fs::read(&destination).unwrap()).unwrap();
        assert_eq!(reopened.canvas, (8, 6));
        assert_eq!(reopened.roots, doc.roots);
        for (y, row) in triangle.iter().enumerate() {
            for (x, inside) in row.bytes().enumerate() {
                let expected = if inside == b'#' {
                    [208, 32, 64, 255]
                } else if complementary {
                    [48, 96, 160, 255]
                } else {
                    [255; 4]
                };
                assert_eq!(
                    reopened.layers[&layer].raster.get(x as i64, y as i64).0,
                    expected,
                    "at {x},{y}, complement={complementary}"
                );
            }
        }
    }
}

#[test]
fn masked_flood_respects_selection_connectivity_and_forbidden_seed() {
    let scratch = Scratch::new("masked-flood");
    let base = scratch.path("base.pcnd");
    let operations = scratch.path("operations.json");
    assert_eq!(
        run(&[
            "new",
            "--width",
            "8",
            "--height",
            "6",
            "--background",
            "#FFFFFFFF",
            "-o",
            &base
        ])
        .code,
        0
    );
    let original = fs::read(&base).unwrap();
    let doc = paintcraft_formats::pcnd::read(&original).unwrap();
    let layer = doc.roots[0];
    let barrier = r#"{"op":"selection.set_polygon","points":[{"x":3,"y":0},{"x":5,"y":0},{"x":5,"y":6},{"x":3,"y":6}]},{"op":"selection.invert"}"#;
    for seed in [0, 4, 7] {
        let destination = scratch.path(&format!("seed-{seed}.pcnd"));
        let flood = format!(
            r##"{{"op":"raster.flood_fill","layer_ref":{{"id":"{layer}"}},"at":{{"x":{seed},"y":2}},"color":"#10A080FF","mode":"replace","tolerance":0}}"##
        );
        fs::write(
            &operations,
            batch(&doc.state_id.to_string(), &format!("{barrier},{flood}")),
        )
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
        assert_eq!(applied.code, 0, "{} {}", applied.stdout, applied.stderr);
        let bytes = fs::read(&destination).unwrap();
        let reopened = paintcraft_formats::pcnd::read(&bytes).unwrap();
        for y in 0..6 {
            for x in 0..8 {
                let reached = (seed == 0 && x < 3) || (seed == 7 && x >= 5);
                let expected = if reached {
                    [16, 160, 128, 255]
                } else {
                    [255; 4]
                };
                assert_eq!(
                    reopened.layers[&layer].raster.get(x, y).0,
                    expected,
                    "seed={seed} at {x},{y}"
                );
            }
        }
        if seed == 4 {
            assert_eq!(
                bytes, original,
                "forbidden seed never paints any source pixel"
            );
            assert_eq!(reopened.state_id, doc.state_id);
        } else {
            assert_ne!(reopened.state_id, doc.state_id);
        }
    }
    assert_eq!(fs::read(base).unwrap(), original);
}
