use super::*;

#[test]
fn layer_expansion_shares_tiles_preserves_padding_and_rolls_back_atomically() {
    let (mut s, id) = session(259, 257);
    run(
        &mut s,
        2,
        &format!(
            "[{}]",
            stroke(id, r#"[{"x":258,"y":256}]"#, "#2040807F", 1, "square")
        ),
    )
    .unwrap();
    let before = s.document().clone();
    let tiles: Vec<_> = before.layers[&id].raster.tile_identities().collect();
    let expand = format!(
        r#"{{"op":"layer.expand","layer_ref":{},"width":300,"height":300}}"#,
        lref(id)
    );
    run(&mut s, 2, &format!("[{expand}]")).unwrap();
    let raster = &s.document().layers[&id].raster;
    assert_eq!(raster.tile_identities().collect::<Vec<_>>(), tiles);
    assert_eq!(raster.get(258, 256), Rgba([32, 64, 128, 127]));
    assert_eq!(raster.get(259, 256), Rgba::TRANSPARENT);
    assert_eq!(raster.get(258, 257), Rgba::TRANSPARENT);
    assert_eq!((raster.width(), raster.height()), (300, 300));
    s.undo().unwrap();
    assert!(s.document().same_semantics(&before));
    let ticket = s.ticket();
    let invalid = expand.replace("300", "1");
    assert_eq!(
        run(&mut s, 2, &format!("[{expand},{invalid}]"))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert_eq!(s.ticket(), ticket);
    assert!(s.document().same_semantics(&before));
    assert_eq!(
        run(&mut s, 1, &format!("[{expand}]")).unwrap_err().code,
        ErrorCode::InputSchema
    );
}
