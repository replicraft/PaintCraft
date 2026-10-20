use super::*;

#[test]
fn clear_is_v2_only_and_rolls_back_if_a_later_operation_fails() {
    let (mut s, id) = session(4, 3);
    let op = r##"{"op":"canvas.clear","ref":"blank","color":"#123456FF"}"##;
    assert_eq!(
        run(&mut s, 1, &format!("[{op}]")).unwrap_err().code,
        ErrorCode::InputSchema
    );
    run(
        &mut s,
        2,
        r#"[{"op":"selection.set_rect","rect":{"x":1,"y":1,"width":1,"height":1}}]"#,
    )
    .unwrap();
    let before = s.snapshot();
    let selection = s.selection();
    let ticket = s.ticket();
    let history = s.undo_depth();
    let failure = run(
        &mut s,
        2,
        &format!(r#"[{op},{{"op":"layer.delete","layer_ref":{}}}]"#, lref(id)),
    )
    .unwrap_err();
    assert_eq!(failure.code, ErrorCode::ReferenceUnknownId);
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.selection(), selection);
    assert_eq!(s.undo_depth(), history);
    assert!(s.document().same_semantics(&before));
    let receipt = run(&mut s, 2, &format!("[{op}]")).unwrap();
    let blank = receipt.aliases["blank"];
    assert_eq!(
        s.document().layers[&blank].raster.get(0, 0),
        Rgba([18, 52, 86, 255])
    );
    assert_eq!(
        s.history_view().entries.last().unwrap().label.text(),
        "Clear Canvas"
    );
}

#[test]
fn clear_invalidates_old_aliases_but_the_new_background_can_be_painted_in_the_same_batch() {
    let (mut s, _) = session(2, 2);
    let add = r##"{"op":"layer.add","ref":"old","name":"Old","width":2,"height":2,"origin":{"x":0,"y":0},"default_pixel":"#00000000"}"##;
    let clear = r##"{"op":"canvas.clear","ref":"blank","color":"#FFFFFF00"}"##;
    let fill = r##"{"op":"raster.fill_rect","layer_ref":{"ref":"blank"},"rect":{"x":0,"y":0,"width":1,"height":1},"color":"#112233FF","mode":"replace"}"##;
    let r = run(&mut s, 2, &format!("[{add},{clear},{fill}]")).unwrap();
    assert!(!r.aliases.contains_key("old"));
    let raster = &s.document().layers[&r.aliases["blank"]].raster;
    assert_eq!(raster.get(0, 0), Rgba([17, 34, 51, 255]));
    assert_eq!(raster.get(1, 1), Rgba([255, 255, 255, 0]));
    let ticket = s.ticket();
    let error = run(&mut s, 2, &format!("[{clear},{clear}]")).unwrap_err();
    assert_eq!(error.code, ErrorCode::InputSchema);
    assert_eq!(s.ticket(), ticket);
}

#[test]
fn clear_handles_more_layers_than_the_batch_operation_limit() {
    let mut doc = new_document(2, 2, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let prototype = doc.layers[&doc.roots[0]].clone();
    for _ in 0..1024 {
        let mut layer = prototype.clone();
        layer.id = Uuid::new_v4();
        doc.roots.push(layer.id);
        doc.layers.insert(layer.id, layer);
    }
    let mut s = Session::new(doc);
    s.set_staged_pixel_budget(0);
    let before = s.snapshot();
    let r = run(
        &mut s,
        2,
        r##"[{"op":"canvas.clear","ref":"blank","color":"#00000000"}]"##,
    )
    .unwrap();
    assert!(r.committed);
    assert_eq!(s.document().roots.len(), 1);
    assert_eq!(
        s.document().layers[&r.aliases["blank"]]
            .raster
            .resident_tiles(),
        0
    );
    s.undo().unwrap().unwrap();
    assert!(s.document().same_semantics(&before));
}
