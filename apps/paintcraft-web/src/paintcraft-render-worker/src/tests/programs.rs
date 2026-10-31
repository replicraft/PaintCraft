use super::*;
use crate::protocol::programs::{CONTROL_BYTES, Fact};
use paintcraft_core::types::filters::ExposureSrgbV1;
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_formats::export::prepared_capabilities::Requirements;

fn fact(requirements: &Requirements) -> Value {
    Fact::capture(requirements, &|| Ok(())).unwrap().value()
}
fn generic_plan(doc: &Document, bytes: &[u8]) -> Value {
    let mut fields = plan(doc, bytes).as_object().unwrap().clone();
    let required = Requirements::capture(doc, &|| Ok(())).unwrap();
    let mut facts = fact(&required).as_object().unwrap().clone();
    facts.insert(
        "pairs".into(),
        Value::Array(
            required
                .pairs()
                .map(|(p, c)| {
                    Value::object([("profile_id", Value::str(p)), ("codec_id", Value::str(c))])
                })
                .collect(),
        ),
    );
    fields.extend([
        ("protocol".into(), Value::Int(5)),
        ("registryContract".into(), Value::Int(1)),
        ("workspaceBytes".into(), Value::Int(64 << 20)),
        ("workLimit".into(), Value::Int(268435456)),
        ("sourceRequirements".into(), Value::Object(facts)),
    ]);
    Value::Object(fields)
}
fn generic_open(doc: &Document) -> (Service, Value) {
    let bytes = pcnd::write(doc).unwrap();
    let p = generic_plan(doc, &bytes);
    (Service::open_programs(&bytes, &text(&p)).unwrap(), p)
}
fn req(
    p: &Value,
    generation: u64,
    target: Option<Uuid>,
    operation: Value,
    candidate: &Requirements,
    union: &Requirements,
    render: bool,
) -> Value {
    let mut v = request(p, generation, target, operation, render, [0, 0, 3, 2], 0)
        .as_object()
        .unwrap()
        .clone();
    v.insert("candidateRequirements".into(), fact(candidate));
    v.insert("unionRequirements".into(), fact(union));
    Value::Object(v)
}
fn exposure(doc: &Document) -> Value {
    FilterOp::Add {
        layer: LayerRef::Id(doc.roots[0]),
        alias: "exposure".into(),
        descriptor: Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(100).unwrap()),
        enabled: true,
        to_index: None,
    }
    .to_value()
    .unwrap()
}
fn exposure_requirements() -> Requirements {
    let mut value = Requirements::default();
    value.insert("exposure-srgb-v1", "params-v1").unwrap();
    value
}
#[test]
fn hello_resolves_only_sorted_requested_subset_and_rejects_unknown_or_duplicate() {
    let pairs = Value::Array(vec![Value::object([
        ("profile_id", Value::str("exposure-srgb-v1")),
        ("codec_id", Value::str("params-v1")),
    ])]);
    let hello = Value::object([
        ("registryContract", Value::Int(1)),
        ("pairs", pairs.clone()),
    ]);
    let reply = Service::capabilities(&text(&hello)).unwrap();
    let entries = field(&reply, "entries").unwrap().as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].get("profile_id"),
        Some(&Value::str("exposure-srgb-v1"))
    );
    let expected = fact(&exposure_requirements());
    for key in ["requiredSha256", "resolutionSha256"] {
        assert_eq!(reply.get(key), expected.get(key));
    }
    let empty = Service::capabilities(r#"{"registryContract":1,"pairs":[]}"#).unwrap();
    assert!(
        field(&empty, "entries")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    for input in [
        r#"{"registryContract":1,"pairs":[{"profile_id":"unknown-v1","codec_id":"params-v1"}]}"#,
        r#"{"registryContract":1,"pairs":[{"profile_id":"invert-rgb-v1","codec_id":"params-v1"},{"profile_id":"invert-rgb-v1","codec_id":"params-v1"}]}"#,
        r#"{"registryContract":2,"pairs":[]}"#,
    ] {
        assert!(Service::capabilities(input).is_err());
    }
    assert_eq!(
        Service::capabilities(&" ".repeat((128 << 10) + 1))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn private_exposure_exact_pixels_bindings_and_source_immutability() {
    let doc = fixture();
    let before = pcnd::write(&doc).unwrap();
    let (mut s, p) = generic_open(&doc);
    let e = exposure_requirements();
    let binding = s
        .prepare(&text(&req(
            &p,
            1,
            Some(doc.roots[0]),
            exposure(&doc),
            &e,
            &e,
            true,
        )))
        .unwrap();
    for key in ["candidateRequirements", "unionRequirements"] {
        assert_eq!(binding.get(key), Some(&fact(&e)));
    }
    assert_eq!(
        binding.get("sourceRequirements"),
        Some(&fact(&Requirements::default()))
    );
    assert_eq!(
        s.frame.as_ref().unwrap().programs.as_ref().unwrap().tiles(),
        1
    );
    let (_, pixels) = finish(&mut s);
    // Independent sRGB EV+1 literals; transparent composition hides source RGB.
    assert_eq!(
        pixels,
        [
            2, 4, 6, 255, 176, 0, 0, 128, 0, 0, 0, 0, 0, 255, 176, 255, 255, 255, 255, 255, 0, 0,
            0, 255
        ]
    );
    s.retire();
    assert_eq!(pcnd::write(&s.base).unwrap(), before);
    assert_eq!(
        s.context().get("sourceRequirements"),
        p.get("sourceRequirements")
    );
}
#[test]
fn immutable_source_requirements_survive_remove_and_candidate_fact_tamper_retires() {
    let mut doc = fixture();
    let layer = doc.roots[0];
    let id = Uuid::from_u128(901);
    doc.layers.get_mut(&layer).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        id,
        false,
        Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(0).unwrap()),
    )])
    .unwrap();
    let (mut s, p) = generic_open(&doc);
    let e = exposure_requirements();
    let empty = Requirements::default();
    let remove = FilterOp::Remove {
        layer: LayerRef::Id(layer),
        filter: FilterRef::Id(id),
    }
    .to_value()
    .unwrap();
    assert_eq!(
        s.prepare(&text(&req(
            &p,
            1,
            Some(layer),
            remove.clone(),
            &empty,
            &empty,
            false
        )))
        .unwrap_err()
        .code,
        ErrorCode::ConflictStaleBase
    );
    let b = s
        .prepare(&text(&req(
            &p,
            2,
            Some(layer),
            remove.clone(),
            &empty,
            &e,
            false,
        )))
        .unwrap();
    assert_eq!(b.get("sourceRequirements"), Some(&fact(&e)));
    assert_eq!(b.get("candidateRequirements"), Some(&fact(&empty)));
    assert!(s.frame.is_none());
    assert_eq!(
        s.prepare(&text(&req(&p, 3, Some(layer), remove, &e, &e, true)))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert!(s.candidate.is_none());
    assert!(s.frame.is_none());
}
#[test]
fn source_digest_requirements_and_control_shape_fail_before_adoption() {
    let doc = fixture();
    let bytes = pcnd::write(&doc).unwrap();
    let p = generic_plan(&doc, &bytes);
    for (key, value) in [
        ("sourceSha256", Value::str("0".repeat(64))),
        ("sourceRequirements", fact(&Requirements::default())),
        ("protocol", Value::Int(4)),
        ("extra", Value::Null),
    ] {
        let mut bad = p.as_object().unwrap().clone();
        bad.insert(key.into(), value);
        assert!(
            Service::open_programs(&bytes, &text(&Value::Object(bad))).is_err(),
            "{key}"
        );
    }
    let mut bad = p.as_object().unwrap().clone();
    let mut requirements = field(&p, "sourceRequirements")
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    requirements.insert("requiredSha256".into(), Value::str("0".repeat(64)));
    bad.insert("sourceRequirements".into(), Value::Object(requirements));
    assert_eq!(
        Service::open_programs(&bytes, &text(&Value::Object(bad)))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
}
#[test]
fn caller_limits_quote_only_and_unknown_owner_never_reset_parent_or_source() {
    use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};
    let doc = fixture();
    let (mut s, p) = generic_open(&doc);
    let e = exposure_requirements();
    let source_scope = s.programs.as_ref().unwrap().scope.clone();
    let baseline = source_scope.footprint().live;
    assert!(baseline > CONTROL_BYTES / 2);
    let budget = SharedBudget::new(64 << 20);
    let scope = budget
        .reserve_capacity(64 << 20)
        .unwrap()
        .scope(AllocationClass::SharedBudget);
    let parent = WorkspaceAdmission::new(scope, 64 << 20, 268435456);
    parent.reserve_work(7).unwrap();
    with_workspace(parent.clone(), || {
        s.prepare(&text(&req(
            &p,
            1,
            Some(doc.roots[0]),
            exposure(&doc),
            &e,
            &e,
            false,
        )))
    })
    .unwrap();
    assert_eq!(parent.work_used(), 7);
    assert!(s.frame.is_none());
    let zero = WorkspaceAdmission::new(parent.scope().clone(), 0, 268435456);
    assert_eq!(
        with_workspace(zero, || s.prepare(&text(&req(
            &p,
            2,
            Some(doc.roots[0]),
            exposure(&doc),
            &e,
            &e,
            true
        ))))
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert!(s.frame.is_none());
    assert!(s.candidate.is_none());
    s.prepare(&text(&req(
        &p,
        3,
        Some(doc.roots[0]),
        exposure(&doc),
        &e,
        &e,
        true,
    )))
    .unwrap();
    let mut foreign = req(
        &p,
        4,
        None,
        Value::Null,
        &Requirements::default(),
        &Requirements::default(),
        true,
    )
    .as_object()
    .unwrap()
    .clone();
    foreign.insert("draft".into(), Value::str(Uuid::nil().to_string()));
    assert_eq!(
        s.prepare(&text(&Value::Object(foreign))).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert!(s.frame.is_some());
    let used = s
        .frame
        .as_ref()
        .unwrap()
        .programs
        .as_ref()
        .unwrap()
        .work_used();
    assert!(used > 0);
    s.retire();
    assert_eq!(source_scope.footprint().live, baseline);
}
#[test]
fn malformed_mask_operation_is_unsupported_then_higher_generation_can_retry() {
    let doc = fixture();
    let (mut s, p) = generic_open(&doc);
    let empty = Requirements::default();
    let bad = Value::object([
        ("op", Value::str("filter.mask.set")),
        ("garbage", Value::Null),
    ]);
    assert_eq!(
        s.prepare(&text(&req(
            &p,
            1,
            Some(doc.roots[0]),
            bad,
            &empty,
            &empty,
            true
        )))
        .unwrap_err()
        .code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(s.candidate.is_none());
    assert!(s.frame.is_none());
    s.prepare(&text(&req(&p, 2, None, Value::Null, &empty, &empty, true)))
        .unwrap();
    finish(&mut s);
}
