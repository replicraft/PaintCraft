use super::*;

const POLYGON: &str =
    r#"{"op":"selection.set_polygon","points":[{"x":1,"y":1},{"x":7,"y":1},{"x":3,"y":5}]}"#;

fn batch(state: &str, operations: &str) -> String {
    format!(
        r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{state}","publication_epoch":"0"}},"operations":[{operations}]}}"#
    )
}

#[test]
fn selection_masks_are_session_only_and_unsupported_paint_is_atomic() {
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
    // A real private paint prefix must also roll back when the later mask gate fails.
    fs::write(
        &operations,
        batch(&state, &format!("{paint},{POLYGON},{paint}")),
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
