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
