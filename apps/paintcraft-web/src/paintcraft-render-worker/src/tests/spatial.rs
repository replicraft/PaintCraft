//! Real private staging, finite work and native-step retirement controls.
use super::*;
use paintcraft_core::types::filters::BoxBlurV1;
use paintcraft_document::{FilterInstance, FilterStack};

fn profile(doc: &Document, bytes: &[u8]) -> Value {
    let mut value = plan(doc, bytes).as_object().unwrap().clone();
    for (key, number) in [
        ("protocol", 3),
        ("spatialCount", protocol::spatial::stored(doc)),
        ("workspaceBytes", protocol::spatial::WORKSPACE_BYTES_MAX),
        ("workLimit", protocol::spatial::WORK_MAX),
    ] {
        value.insert(key.into(), Value::Int(number as i64));
    }
    Value::Object(value)
}
fn spatial_source(width: u32, height: u32, radius: u8, enabled: bool) -> Document {
    let mut doc = paintcraft_document::import_image(
        width,
        height,
        &[20, 40, 60, 255].repeat(width as usize * height as usize),
        "uniform-box",
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(71),
            enabled,
            Descriptor::BoxBlurV1(BoxBlurV1::new(i16::from(radius)).unwrap()),
        )])
        .unwrap();
    doc
}
fn add_box(doc: &Document, radius: u8) -> Value {
    FilterOp::Add {
        layer: LayerRef::Id(doc.roots[0]),
        alias: "box".into(),
        descriptor: Descriptor::BoxBlurV1(BoxBlurV1::new(i16::from(radius)).unwrap()),
        enabled: true,
        to_index: None,
    }
    .to_value()
    .unwrap()
}
fn open3(doc: &Document) -> (Service, Value) {
    let bytes = pcnd::write(doc).unwrap();
    let control = profile(doc, &bytes);
    (Service::open(&bytes, &text(&control)).unwrap(), control)
}
#[test]
fn legacy_profile_is_closed_and_refuses_stored_disabled_radius_zero_and_private_box() {
    for enabled in [false, true] {
        let doc = spatial_source(1, 1, 0, enabled);
        let bytes = pcnd::write(&doc).unwrap();
        assert_eq!(bytes[4], 5);
        assert_eq!(
            Service::open(&bytes, &text(&plan(&doc, &bytes)))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    let doc = fixture();
    let (mut service, control) = open(&doc);
    let old_quote = service.source.quote;
    assert!(service.source.spatial.is_none());
    for key in ["spatialCount", "workspaceBytes", "workLimit"] {
        let mut changed = control.as_object().unwrap().clone();
        changed.insert(key.into(), Value::Int(0));
        assert_eq!(
            Source::parse(Value::Object(changed)).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
    let req = request(
        &control,
        1,
        Some(doc.roots[0]),
        add_box(&doc, 0),
        true,
        [0, 0, 3, 2],
        0,
    );
    assert_eq!(
        service.prepare(&text(&req)).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(service.frame.is_none());
    assert!(service.candidate.is_none());
    assert_eq!(service.source.quote, old_quote);
    service
        .prepare(&text(&request(
            &control,
            2,
            None,
            Value::Null,
            true,
            [0, 0, 3, 2],
            0,
        )))
        .unwrap();
    assert_eq!(
        finish(&mut service).1,
        paintcraft_render::render_flat(&doc).unwrap()
    );
}
#[test]
fn explicit_profile_admits_box_add_and_prepared_without_mutating_frozen_source() {
    let doc = paintcraft_document::import_image(
        3,
        1,
        &[255, 0, 0, 255, 0, 0, 255, 0, 0, 0, 255, 0],
        "red-transparent",
    )
    .unwrap();
    let original = pcnd::write(&doc).unwrap();
    let (mut service, control) = open3(&doc);
    assert_eq!(service.context(), control);
    let mut req = request(
        &control,
        1,
        Some(doc.roots[0]),
        add_box(&doc, 1),
        false,
        [0, 0, 3, 1],
        0,
    );
    assert_eq!(
        service.prepare(&text(&req)).unwrap().get("kind"),
        Some(&Value::str("prepared"))
    );
    assert!(service.frame.is_none());
    let mut fields = req.as_object().unwrap().clone();
    fields.insert("generation".into(), Value::str("2"));
    fields.insert("render".into(), Value::Bool(true));
    req = Value::Object(fields);
    service.prepare(&text(&req)).unwrap();
    // CLAMP: two red samples then one red sample; hidden blue contributes no color.
    assert_eq!(
        finish(&mut service).1,
        [255, 0, 0, 170, 255, 0, 0, 85, 0, 0, 0, 0]
    );
    assert_eq!(pcnd::write(&service.base).unwrap(), original);
}
#[test]
fn native_lod_is_one_tile_per_step_cancel_discards_partial_and_resets_job_counter() {
    let doc = spatial_source(256, 256, 1, true);
    let original = pcnd::write(&doc).unwrap();
    let (mut service, control) = open3(&doc);
    let req = |n| request(&control, n, None, Value::Null, true, [0, 0, 256, 256], 8);
    service.prepare(&text(&req(1))).unwrap();
    assert_eq!(
        service
            .frame
            .as_ref()
            .unwrap()
            .spatial
            .as_ref()
            .unwrap()
            .tiles(),
        16
    );
    assert!(!service.step().unwrap());
    let used = service
        .frame
        .as_ref()
        .unwrap()
        .spatial
        .as_ref()
        .unwrap()
        .work_used();
    assert!(used > 0 && used <= protocol::spatial::NATIVE_STEP_WORK_MAX);
    assert!(service.take_frame().is_err());
    service.retire();
    assert!(service.frame.is_none() && service.candidate.is_none());
    service.prepare(&text(&req(2))).unwrap();
    assert_eq!(
        service
            .frame
            .as_ref()
            .unwrap()
            .spatial
            .as_ref()
            .unwrap()
            .work_used(),
        0
    );
    let mut steps = 0;
    loop {
        steps += 1;
        if service.step().unwrap() {
            break;
        }
    }
    assert_eq!(steps, 16);
    assert_eq!(service.take_frame().unwrap().1, [20, 40, 60, 255]);
    assert_eq!(pcnd::write(&service.base).unwrap(), original);
}
#[test]
fn spatial_facts_and_workspace_work_limits_fail_before_output_allocation() {
    let doc = spatial_source(2, 1, 1, true);
    let bytes = pcnd::write(&doc).unwrap();
    let good = profile(&doc, &bytes);
    for (key, value, expected) in [
        ("spatialCount", 0, ErrorCode::ConflictStaleBase),
        ("protocol", 2, ErrorCode::InputSchema),
        ("protocol", 4, ErrorCode::InputSchema),
        ("workspaceBytes", (64 << 20) + 1, ErrorCode::ResourceLimit),
        ("workLimit", 268435457, ErrorCode::ResourceLimit),
    ] {
        let mut fields = good.as_object().unwrap().clone();
        fields.insert(key.into(), Value::Int(value));
        assert_eq!(
            Service::open(&bytes, &text(&Value::Object(fields)))
                .unwrap_err()
                .code,
            expected
        );
    }
    for key in ["workspaceBytes", "workLimit"] {
        let mut fields = good.as_object().unwrap().clone();
        fields.insert(key.into(), Value::Int(0));
        let control = Value::Object(fields);
        let mut service = Service::open(&bytes, &text(&control)).unwrap();
        let error = service
            .prepare(&text(&request(
                &control,
                1,
                None,
                Value::Null,
                true,
                [0, 0, 2, 1],
                0,
            )))
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert!(service.frame.is_none());
    }
    // A point-only prefix is included in profile3 work even with no stored Box.
    let doc = fixture();
    let bytes = pcnd::write(&doc).unwrap();
    let mut fields = profile(&doc, &bytes).as_object().unwrap().clone();
    fields.insert("workLimit".into(), Value::Int(6));
    let control = Value::Object(fields);
    let mut service = Service::open(&bytes, &text(&control)).unwrap();
    assert_eq!(
        service
            .prepare(&text(&request(
                &control,
                1,
                Some(doc.roots[0]),
                invert(&doc, true),
                true,
                [0, 0, 3, 2],
                0
            )))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(service.frame.is_none());
}

#[test]
fn single_native_step_includes_all_point_layers_before_any_frame_allocation() {
    let mut doc =
        paintcraft_document::import_image(64, 64, &[5, 10, 15, 255].repeat(64 * 64), "point-work")
            .unwrap();
    let original_layer = doc.layers[&doc.roots[0]].clone();
    doc.layers.clear();
    doc.roots.clear();
    // Level-zero patches are 32 native pixels wide, so 16 point layers
    // exceed the step profile even though each independent layer fits.
    for index in 0..16 {
        let mut layer = original_layer.clone();
        layer.id = Uuid::from_u128(index + 1);
        layer.filters = FilterStack::new(
            (0..256)
                .map(|i| {
                    FilterInstance::new(Uuid::from_u128(i + 100), true, Descriptor::InvertRgbV1)
                })
                .collect(),
        )
        .unwrap();
        doc.roots.push(layer.id);
        doc.layers.insert(layer.id, layer);
    }
    let (mut service, control) = open3(&doc);
    let error = service
        .prepare(&text(&request(
            &control,
            1,
            None,
            Value::Null,
            true,
            [0, 0, 64, 64],
            0,
        )))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(error.message.contains("single-step"));
    assert!(service.frame.is_none());
}

#[path = "spatial/version_guards.rs"]
mod version_guards;
