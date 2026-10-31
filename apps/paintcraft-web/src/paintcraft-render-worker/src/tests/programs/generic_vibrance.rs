use super::*;
use paintcraft_core::types::filters::VibranceRgbV1;
fn descriptor(a: i16) -> Descriptor {
    Descriptor::VibranceRgbV1(VibranceRgbV1::new(a).unwrap())
}
fn scene() -> Document {
    let mut d = paintcraft_document::import_image(
        2,
        1,
        &[20, 115, 200, 255, 64, 16, 128, 255],
        "generic-v",
    )
    .unwrap();
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        Uuid::from_u128(871),
        true,
        descriptor(100),
    )])
    .unwrap();
    d
}
fn control(
    p: &Value,
    generation: u64,
    operation: Value,
    candidate: &Requirements,
    union: &Requirements,
    render: bool,
) -> Value {
    let mut r = req(
        p,
        generation,
        if operation == Value::Null {
            None
        } else {
            Some("00000000-0000-0000-0000-000000000000".parse().unwrap())
        },
        operation,
        candidate,
        union,
        render,
    )
    .as_object()
    .unwrap()
    .clone();
    r.insert(
        "view".into(),
        Value::object([
            (
                "region",
                Value::Array(vec![
                    Value::Int(0),
                    Value::Int(0),
                    Value::Int(2),
                    Value::Int(1),
                ]),
            ),
            ("level", Value::Int(0)),
        ]),
    );
    Value::Object(r)
}
#[test]
fn source5_direct_base_quotes_without_lut_and_spends_actual64_not_double_bulk() {
    let d = scene();
    let before = pcnd::write(&d).unwrap();
    let required = Requirements::capture(&d, &|| Ok(())).unwrap();
    let mut p = generic_plan(&d, &before).as_object().unwrap().clone();
    p.insert("workLimit".into(), Value::Int(274));
    let p = Value::Object(p);
    let mut s = Service::open_programs(&before, &text(&p)).unwrap();
    s.prepare(&text(&control(
        &p,
        1,
        Value::Null,
        &required,
        &required,
        false,
    )))
    .unwrap();
    assert!(s.frame.is_none());
    s.prepare(&text(&control(
        &p,
        2,
        Value::Null,
        &required,
        &required,
        true,
    )))
    .unwrap();
    assert_eq!(
        s.frame
            .as_ref()
            .unwrap()
            .programs
            .as_ref()
            .unwrap()
            .work_used(),
        18
    );
    while !s.step().unwrap() {}
    assert_eq!(
        s.frame
            .as_ref()
            .unwrap()
            .programs
            .as_ref()
            .unwrap()
            .work_used(),
        18 + 128
    );
    let (_, pixels) = s.take_frame().unwrap();
    assert_eq!(pixels, [4, 116, 216, 255, 62, 4, 140, 255]);
    s.retire();
    assert_eq!(pcnd::write(&s.base).unwrap(), before);
}
#[test]
fn source5_quote_only_and_render_refuse_one_below_bound_before_output_then_retry() {
    let d = scene();
    let before = pcnd::write(&d).unwrap();
    let required = Requirements::capture(&d, &|| Ok(())).unwrap();
    for render in [false, true] {
        let mut p = generic_plan(&d, &before).as_object().unwrap().clone();
        p.insert("workLimit".into(), Value::Int(273));
        let p = Value::Object(p);
        let mut s = Service::open_programs(&before, &text(&p)).unwrap();
        assert_eq!(
            s.prepare(&text(&control(
                &p,
                1,
                Value::Null,
                &required,
                &required,
                render
            )))
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit
        );
        assert!(s.frame.is_none());
        assert_eq!(pcnd::write(&s.base).unwrap(), before);
    }
    let (mut s, p) = generic_open(&d);
    s.prepare(&text(&control(
        &p,
        1,
        Value::Null,
        &required,
        &required,
        true,
    )))
    .unwrap();
    s.retire();
    assert!(s.frame.is_none());
    assert_eq!(pcnd::write(&s.base).unwrap(), before);
    s.prepare(&text(&control(
        &p,
        2,
        Value::Null,
        &required,
        &required,
        true,
    )))
    .unwrap();
    assert_eq!(finish(&mut s).1, [4, 116, 216, 255, 62, 4, 140, 255]);
}
#[test]
fn source5_private_update_allows_identity_intent_and_checks_same_parent_min_cap() {
    use paintcraft_core::types::resources::AllocationClass;
    use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};
    let d = scene();
    let before = pcnd::write(&d).unwrap();
    let required = Requirements::capture(&d, &|| Ok(())).unwrap();
    let (mut s, p) = generic_open(&d);
    let operation = FilterOp::Update {
        layer: LayerRef::Id(d.roots[0]),
        filter: FilterRef::Id(Uuid::from_u128(871)),
        descriptor: descriptor(0),
    }
    .to_value()
    .unwrap();
    let mut r = control(&p, 1, operation, &required, &required, true)
        .as_object()
        .unwrap()
        .clone();
    r.insert("target".into(), Value::str(d.roots[0].to_string()));
    let r = Value::Object(r);
    let b = SharedBudget::new(1 << 20);
    let a = WorkspaceAdmission::new(b.scope(AllocationClass::SharedBudget), 1 << 20, 273);
    assert_eq!(
        with_workspace(a.clone(), || s.prepare(&text(&r)))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(a.work_used(), 0);
    let mut r = r.as_object().unwrap().clone();
    r.insert("generation".into(), Value::str("2"));
    s.prepare(&text(&Value::Object(r))).unwrap();
    while !s.step().unwrap() {}
    assert_eq!(
        s.frame
            .as_ref()
            .unwrap()
            .programs
            .as_ref()
            .unwrap()
            .work_used(),
        146
    );
    assert_eq!(
        s.take_frame().unwrap().1,
        [20, 115, 200, 255, 64, 16, 128, 255]
    );
    assert_eq!(pcnd::write(&s.base).unwrap(), before);
}
