use super::*;
const SOURCE: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/csh-fixtures/placement.csh"
);
fn editor() -> Editor {
    let mut e =
        Editor::from_document(paintcraft_document::new_document(32, 32, None, 0, POLICY).unwrap())
            .unwrap();
    e.use_resource_shape(
        Reference {
            library: Uuid::from_u128(7),
            preset: 1,
            policy: Policy::CubicFillV1.id(),
        },
        Arc::new(csh::decode(SOURCE, &|| Ok(())).unwrap()),
        "Lens".into(),
    )
    .unwrap();
    e
}
#[test]
fn browser_drag_budget_and_missing_owner_refuse_entire_preview_release_without_partial_pixels() {
    let mut e = editor();
    let before = e.session.snapshot();
    e.tools.pointer_down(Uuid::nil(), (0, 0));
    e.tools.pointer_move((512, 512));
    assert_eq!(
        e.display_document().unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let (_, action) = e.tools.pointer_up().unwrap();
    let error = e.apply(action).unwrap_err();
    assert_eq!(
        error.message,
        "Custom shape size or position exceeds the drawing limit"
    );
    assert!(Arc::ptr_eq(&before, &e.session.snapshot()));
    assert_eq!(e.session.undo_depth(), 0);
    assert!(e.session.document().layers.is_empty());
    e.tools.pointer_down(Uuid::nil(), (-240, -240));
    e.tools.pointer_move((271, 271));
    assert!(e.display_document().unwrap().is_some());
    e.cancel();
    assert!(e.tools.pointer_up().is_none());
    assert!(Arc::ptr_eq(&before, &e.session.snapshot()));
    e.tools.color = Rgba::TRANSPARENT;
    e.tools.pointer_down(Uuid::nil(), (1, 1));
    let (_, action) = e.tools.pointer_up().unwrap();
    e.apply(action).unwrap();
    assert!(Arc::ptr_eq(&before, &e.session.snapshot()));
    e.tools.pointer_down(Uuid::nil(), (1, 1));
    e.tools.pointer_move((10, 10));
    e.resource_shape = None;
    assert_eq!(
        e.display_document().unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    let (_, action) = e.tools.pointer_up().unwrap();
    assert!(e.apply(action).is_err());
    assert!(Arc::ptr_eq(&before, &e.session.snapshot()));
}
#[test]
fn browser_picker_restores_exact_retained_source_and_hidden_layers_cannot_receive_shapes() {
    use paintcraft_core::types::picker::PickerAfter;
    let mut e = editor();
    e.tools.options.picker_after = PickerAfter::Previous;
    e.tools.options.picker_source = paintcraft_core::types::picker::SampleSource::Image;
    e.tools.choose_tool(Tool::ColorPicker);
    e.tools.pointer_down(Uuid::nil(), (0, 0));
    let (_, pick) = e.tools.pointer_up().unwrap();
    e.apply(pick).unwrap();
    assert_eq!(e.resource_shape_name(), Some("Lens"));
    e.tools.choose_tool(Tool::ColorPicker);
    e.tools.pointer_down(Uuid::nil(), (0, 0));
    let (_, pick) = e.tools.pointer_up().unwrap();
    e.resource_shape = None;
    assert!(e.apply(pick).is_err());
    assert_eq!(e.tools.tool, Tool::ColorPicker);
    let mut e = editor();
    e.tools.pointer_down(Uuid::nil(), (1, 1));
    e.tools.pointer_move((5, 5));
    let (_, action) = e.tools.pointer_up().unwrap();
    e.apply(action).unwrap();
    let layer = e.target.unwrap();
    e.commit(vec![Value::object([
        ("op", Value::str("layer.set_properties")),
        ("layer_ref", layer_ref(layer)),
        ("visible", Value::Bool(false)),
    ])])
    .unwrap();
    let before = e.session.snapshot();
    e.tools.pointer_down(layer, (1, 1));
    e.tools.pointer_move((4, 4));
    assert!(e.display_document().is_err());
    let (_, action) = e.tools.pointer_up().unwrap();
    assert_eq!(e.apply(action).unwrap_err().code, ErrorCode::TargetInvalid);
    assert!(Arc::ptr_eq(&before, &e.session.snapshot()));
}
