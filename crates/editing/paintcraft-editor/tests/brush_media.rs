use paintcraft_commands::{Batch, Mode, Op};
use paintcraft_core::types::{
    Rect, Rgba,
    brush::BrushMedia,
    json::{self, Value},
};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::{
    execute::{self, Selection},
    session::Session,
};

#[test]
fn each_medium_parses_executes_clips_undoes_and_preserves_hidden_rgb() {
    for media in BrushMedia::ALL.into_iter().skip(1) {
        let doc = new_document(64, 48, Some(Rgba([11, 22, 33, 0])), 0, POLICY).unwrap();
        let id = doc.roots[0];
        let selection = Rect::new(20, 15, 16, 18).unwrap();
        let raw = Value::object([
            ("schema", Value::str("paintcraft.batch/v2")),
            ("policy", Value::str(POLICY)),
            ("atomic", Value::Bool(true)),
            (
                "base",
                Value::object([
                    ("state_id", Value::str(doc.state_id.to_string())),
                    ("publication_epoch", Value::str("0")),
                ]),
            ),
            (
                "operations",
                Value::Array(vec![Value::object([
                    ("op", Value::str("raster.stroke_path")),
                    (
                        "layer_ref",
                        Value::object([("id", Value::str(id.to_string()))]),
                    ),
                    (
                        "points",
                        Value::Array(vec![
                            Value::object([
                                ("x", Value::Int(10 * 256)),
                                ("y", Value::Int(24 * 256)),
                            ]),
                            Value::object([
                                ("x", Value::Int(48 * 256)),
                                ("y", Value::Int(24 * 256)),
                            ]),
                        ]),
                    ),
                    ("color", Value::str("#88442280")),
                    ("size", Value::Int(16)),
                    ("mode", Value::str("source-over")),
                    ("point_scale", Value::Int(256)),
                    ("brush", Value::str(media.id())),
                ])]),
            ),
        ]);
        let batch = Batch::from_json(&json::canonical(&raw)).unwrap();
        let before = doc.clone();
        let staged = execute::execute(&doc, Selection::Rect(selection), &batch).unwrap();
        assert!(doc.same_semantics(&before));
        let mut touched = 0;
        for y in 0..48 {
            for x in 0..64 {
                let pixel = staged.doc.layers[&id].raster.get(x, y);
                if !selection.contains(x, y) {
                    assert_eq!(pixel, Rgba([11, 22, 33, 0]));
                }
                if pixel.a() != 0 {
                    touched += 1;
                    assert!(pixel.a() <= 128);
                    assert_eq!(&pixel.0[..3], &[136, 68, 34]);
                }
            }
        }
        assert!(touched > 0, "{media:?}");
        let mut session = Session::new(doc.clone());
        session.apply(&batch).unwrap();
        session.undo().unwrap();
        assert!(session.document().same_semantics(&doc));
        session.redo().unwrap();
        assert!(!session.document().same_semantics(&doc));
        let mut invalid = batch.clone();
        let Op::StrokePath { mode, .. } = &mut invalid.operations[0] else {
            unreachable!()
        };
        *mode = Mode::Replace;
        assert!(execute::execute(&doc, Selection::Inactive, &invalid).is_err());
    }
}

#[test]
fn all_nine_brushes_multiply_selected_opacity_once_even_at_the_end_of_long_strokes() {
    let doc = new_document(640, 64, Some(Rgba([11, 22, 33, 0])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let selection = Selection::Rect(Rect::new(20, 16, 600, 32).unwrap());
    for media in BrushMedia::ALL {
        let mut batch = Batch {
            version: 2,
            policy: POLICY.into(),
            base_state: doc.state_id,
            base_epoch: 0,
            seed: None,
            digest: "0".repeat(64),
            operations: vec![Op::StrokePath {
                layer: paintcraft_commands::LayerRef::Id(id),
                points: vec![(16, 32), (620, 32), (16, 32)],
                color: Rgba([80, 40, 20, 255]),
                mode: Mode::SourceOver,
                size: 16,
                round: true,
                smooth: media == BrushMedia::Plain,
                media,
                point_scale: 1,
            }],
        };
        let full = execute::execute(&doc, selection.clone(), &batch).unwrap();
        for alpha in [0, 64, 128, 255] {
            let Op::StrokePath { color, .. } = &mut batch.operations[0] else {
                unreachable!()
            };
            color.0[3] = alpha;
            let result = execute::execute(&doc, selection.clone(), &batch).unwrap();
            for y in 0..64 {
                for x in 0..640 {
                    let source = full.doc.layers[&id].raster.get(x, y);
                    let actual = result.doc.layers[&id].raster.get(x, y);
                    let expected = ((u16::from(source.a()) * u16::from(alpha) + 127) / 255) as u8;
                    // Plain uses exact 16-sample coverage before integer alpha
                    // rounding. Its quantized 8-bit reference can differ by one.
                    assert!(
                        actual.a().abs_diff(expected) <= u8::from(media == BrushMedia::Plain),
                        "{media:?} alpha={alpha} at {x},{y}"
                    );
                    assert_eq!(
                        &actual.0[..3],
                        if actual.a() == 0 {
                            &[11, 22, 33]
                        } else {
                            &[80, 40, 20]
                        }
                    );
                }
            }
            if matches!(media, BrushMedia::Oil | BrushMedia::Watercolor) && alpha > 0 {
                assert!((300..600).all(|x| result.doc.layers[&id].raster.get(x, 32).a() > 0));
            }
        }
        assert_eq!(doc.layers[&id].raster.get(300, 32), Rgba([11, 22, 33, 0]));
    }
}
