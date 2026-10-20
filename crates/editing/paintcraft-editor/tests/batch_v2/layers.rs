use super::*;
use paintcraft_raster::{Admission, RasterLayer, Selection};

#[test]
fn duplicate_preserves_complete_layer_and_shares_tiles_without_pixel_admission() {
    let mut doc = new_document(3, 2, Some(Rgba([9, 8, 7, 255])), 10, POLICY).unwrap();
    let source = doc.roots[0];
    let layer = doc.layers.get_mut(&source).unwrap();
    layer.origin = (-2, -1);
    layer.visible = false;
    layer.opacity = 93;
    layer.raster = RasterLayer::from_rgba(
        2,
        2,
        Rgba([3, 2, 1, 0]),
        &[12, 34, 56, 0, 100, 110, 120, 127, 0, 1, 2, 255, 4, 5, 6, 17],
        &mut Admission::new(1 << 20),
    )
    .unwrap();
    let original = layer.clone();
    let mut s = Session::new(doc);
    run(
        &mut s,
        2,
        r#"[{"op":"selection.set_rect","rect":{"x":100,"y":100,"width":1,"height":1}}]"#,
    )
    .unwrap();
    assert_eq!(s.selection(), Selection::Empty);
    let before = s.snapshot();
    let selection = s.selection();
    s.set_staged_pixel_budget(0);
    let op = format!(
        r#"[{{"op":"layer.duplicate","layer_ref":{},"ref":"copy","name":"Copy"}}]"#,
        lref(source)
    );
    let mut replay = Session::new((*before).clone());
    run(
        &mut replay,
        2,
        r#"[{"op":"selection.set_rect","rect":{"x":100,"y":100,"width":1,"height":1}}]"#,
    )
    .unwrap();
    let receipt = run(&mut s, 2, &op).unwrap();
    let copy = receipt.aliases["copy"];
    assert_eq!(copy, run(&mut replay, 2, &op).unwrap().aliases["copy"]);
    assert_ne!(copy, source);
    assert_eq!(s.document().roots, vec![source, copy]);
    assert_eq!(s.document().layers[&source], original);
    let mut expected = original.clone();
    expected.id = copy;
    expected.name = "Copy".into();
    assert_eq!(s.document().layers[&copy], expected);
    assert_eq!(
        s.document().layers[&source]
            .raster
            .tile_identities()
            .collect::<std::collections::HashSet<_>>(),
        s.document().layers[&copy]
            .raster
            .tile_identities()
            .collect::<std::collections::HashSet<_>>()
    );
    assert_eq!(s.selection(), selection);
    assert_eq!(
        s.history_view().entries.last().unwrap().label,
        paintcraft_history::Label::DuplicateLayer
    );
    let duplicated = s.snapshot();
    s.undo().unwrap().unwrap();
    assert!(s.document().same_semantics(&before));
    s.redo().unwrap().unwrap();
    assert!(s.document().same_semantics(&duplicated));
}

#[test]
fn duplicate_is_inserted_above_source_and_copy_writes_are_independent() {
    let (mut s, source) = session(4, 4);
    run(&mut s, 2, r##"[{"op":"layer.add","ref":"top","name":"Top","width":4,"height":4,"origin":{"x":0,"y":0},"default_pixel":"#00000000"}]"##).unwrap();
    let top = s.document().roots[1];
    let r = run(&mut s, 2, &format!(r##"[
        {{"op":"layer.duplicate","layer_ref":{},"ref":"copy","name":"Copy"}},
        {{"op":"raster.fill_rect","layer_ref":{{"ref":"copy"}},"rect":{{"x":0,"y":0,"width":1,"height":1}},"color":"#FF0000FF","mode":"replace"}}
    ]"##,lref(source))).unwrap();
    let copy = r.aliases["copy"];
    assert_eq!(s.document().roots, vec![source, copy, top]);
    assert_eq!(
        s.document().layers[&source].raster.get(0, 0),
        Rgba::TRANSPARENT
    );
    assert_eq!(
        s.document().layers[&copy].raster.get(0, 0),
        Rgba([255, 0, 0, 255])
    );
    s.undo().unwrap().unwrap();
    assert_eq!(s.document().roots, vec![source, top]);
    s.redo().unwrap().unwrap();
    assert_eq!(
        s.document().layers[&copy].raster.get(0, 0),
        Rgba([255, 0, 0, 255])
    );
}

#[test]
fn duplicate_failures_leave_the_batch_history_and_source_unpublished() {
    let (mut s, source) = session(2, 2);
    let op = format!(
        r#"{{"op":"layer.duplicate","layer_ref":{},"ref":"copy","name":"Copy"}}"#,
        lref(source)
    );
    let before = s.snapshot();
    let ticket = s.ticket();
    let history = s.history_view().entries;
    assert_eq!(
        run(&mut s, 1, &format!("[{op}]")).unwrap_err().code,
        ErrorCode::InputSchema
    );
    for tail in [
        op.clone(),
        r#"{"op":"layer.delete","layer_ref":{"ref":"missing"}}"#.into(),
    ] {
        assert!(run(&mut s, 2, &format!("[{op},{tail}]")).is_err());
        assert!(s.document().same_semantics(&before));
        assert_eq!(s.ticket(), ticket);
        assert_eq!(s.history_view().entries, history);
    }
    s.set_staged_pixel_budget(0);
    let fill = r##"{"op":"raster.fill_rect","layer_ref":{"ref":"copy"},"rect":{"x":0,"y":0,"width":1,"height":1},"color":"#123456FF","mode":"replace"}"##;
    assert_eq!(
        run(&mut s, 2, &format!("[{op},{fill}]")).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(s.ticket(), ticket);
    assert!(s.document().same_semantics(&before));
}
