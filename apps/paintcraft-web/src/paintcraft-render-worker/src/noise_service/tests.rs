use super::*;
use paintcraft_core::types::{
    filters::{AddNoiseRgbV1, BoxBlurV1, Descriptor, NoiseDistribution},
    ids::deterministic_id,
};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_formats::export::noise_workload::Workload;
use paintcraft_render::noise_work::{NoiseJobPolicy, NoiseLimits};
fn doc(distribution: NoiseDistribution, blur: Option<u8>) -> Document {
    let mut d = paintcraft_document::new_document(
        2,
        1,
        Some(paintcraft_core::types::Rgba([64, 16, 128, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let mut filters = vec![FilterInstance::new(
        deterministic_id(&["Source6 Noise"]),
        true,
        Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(100, 65535, false, distribution).unwrap()),
    )];
    if let Some(r) = blur {
        filters.push(FilterInstance::new(
            deterministic_id(&["Source6 Box"]),
            true,
            Descriptor::BoxBlurV1(BoxBlurV1::new(i16::from(r)).unwrap()),
        ));
    }
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(filters).unwrap();
    d
}
fn full(required: &Requirements, workload: Workload) -> Value {
    let Value::Object(mut v) = wire::Fact::capture(required, workload, &|| Ok(()))
        .unwrap()
        .value()
    else {
        unreachable!()
    };
    v.insert(
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
    Value::Object(v)
}
fn source(d: &Document, limits: NoiseLimits) -> (Vec<u8>, Value) {
    let bytes = pcnd::write(d).unwrap();
    let r = Requirements::capture(d, &|| Ok(())).unwrap();
    let inv = filter_bake::inspect(d, &|| Ok(())).unwrap();
    let masks = pcnd::coverage_facts(d, &|| Ok(())).unwrap();
    let resident = d
        .layers
        .values()
        .map(|l| {
            l.raster.resident_tiles() as u64 * paintcraft_raster::RasterLayer::TILE_BYTES as u64
        })
        .sum::<u64>();
    let workload = Workload::browser(limits).unwrap();
    let v = Value::object([
        (
            "owner",
            Value::object([
                (
                    "session",
                    Value::str(deterministic_id(&["Source6 owner"]).to_string()),
                ),
                ("state", Value::str(d.state_id.to_string())),
                ("epoch", Value::str("0")),
                ("frontend", Value::Int(3)),
            ]),
        ),
        (
            "draft",
            Value::str(deterministic_id(&["Source6 draft"]).to_string()),
        ),
        ("document", Value::str(d.document_id.to_string())),
        ("sourceSha256", Value::str(hex(&Sha256::digest(&bytes)))),
        (
            "size",
            Value::Array(vec![
                Value::Int(i64::from(d.canvas.0)),
                Value::Int(i64::from(d.canvas.1)),
            ]),
        ),
        ("layers", Value::Int(d.layers.len() as i64)),
        ("residentBytes", Value::Int(resident as i64)),
        ("metadataBytes", Value::Int(0)),
        ("metadataCharge", Value::Int(0)),
        ("filterDigest", Value::str(hex(&inv.digest()))),
        ("maskCount", Value::Int(masks.masks as i64)),
        ("maskBands", Value::Int(masks.bands as i64)),
        ("maskRuns", Value::Int(masks.runs as i64)),
        ("maskDecodeBytes", Value::Int(masks.decode_bytes as i64)),
        ("protocol", Value::Int(6)),
        ("registryContract", Value::Int(2)),
        ("sourceRequirements", full(&r, workload)),
        ("workload", workload.value().unwrap()),
    ]);
    (bytes, v)
}
fn request(service: &NoiseService, generation: u64, render: bool) -> Value {
    let s = &service.state;
    let r = s.required.requirements();
    let fact = wire::Fact::capture(r, s.profile.workload, &|| Ok(()))
        .unwrap()
        .value();
    Value::object([
        ("protocol", Value::Int(6)),
        ("registryContract", Value::Int(2)),
        ("owner", field(&s.source.value, "owner").unwrap().clone()),
        ("draft", field(&s.source.value, "draft").unwrap().clone()),
        (
            "sourceSha256",
            field(&s.source.value, "sourceSha256").unwrap().clone(),
        ),
        ("generation", Value::str(generation.to_string())),
        ("operation", Value::Null),
        ("target", Value::Null),
        (
            "view",
            Value::object([
                (
                    "region",
                    Value::Array(vec![
                        Value::Int(0),
                        Value::Int(0),
                        Value::Int(i64::from(s.base.canvas.0)),
                        Value::Int(i64::from(s.base.canvas.1)),
                    ]),
                ),
                ("level", Value::Int(0)),
            ]),
        ),
        ("render", Value::Bool(render)),
        ("candidateRequirements", fact.clone()),
        ("unionRequirements", fact),
        (
            "workloadSha256",
            Value::str(hex(&s.profile.workload.digest().unwrap())),
        ),
    ])
}
fn open(d: &Document, limits: NoiseLimits) -> NoiseService {
    let (bytes, p) = source(d, limits);
    NoiseService::open(&bytes, &text(&p), SlicePolicy::selected_qualification()).unwrap()
}
fn finish(s: &mut NoiseService) {
    for _ in 0..100_000 {
        if s.step().unwrap() {
            return;
        }
    }
    panic!("finite Source6 frame did not finish");
}
fn set(v: &mut Value, key: &str, value: Value) {
    let Value::Object(fields) = v else {
        panic!("object")
    };
    fields.insert(key.into(), value);
}
#[test]
fn actual_both_noise_fields_and_output_accounts_survive_service_close_until_packet_drop() {
    for (distribution, expected) in [
        (
            NoiseDistribution::Uniform,
            [255, 211, 0, 255, 51, 0, 97, 255],
        ),
        (
            NoiseDistribution::Gaussian,
            [47, 139, 0, 255, 0, 4, 46, 255],
        ),
    ] {
        let d = doc(distribution, None);
        let before = pcnd::write(&d).unwrap();
        let mut service = open(&d, NoiseJobPolicy::BrowserV1.max_limits());
        let budget = service.state.workspace.budget();
        let p = request(&service, 1, true);
        let binding = service.prepare(&text(&p)).unwrap();
        assert_eq!(
            service.take_frame().unwrap_err().code,
            ErrorCode::InputSchema
        );
        finish(&mut service);
        let packet = service.take_frame().unwrap();
        assert_eq!(packet.binding(), &binding);
        assert_eq!(packet.pixels(), expected);
        assert_eq!(pcnd::write(&service.state.base).unwrap(), before);
        drop(service);
        assert!(budget.usage().reserved > 0);
        assert_eq!(packet.pixels(), expected);
        drop(packet);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn quote_only_has_no_frame_sidecar_or_pixel_allocation_and_keeps_source_exact() {
    let d = doc(NoiseDistribution::Gaussian, None);
    let mut service = open(&d, NoiseJobPolicy::BrowserV1.max_limits());
    let budget = service.state.workspace.budget();
    let before = budget.usage().reserved;
    let bytes = pcnd::write(&service.state.base).unwrap();
    let p = request(&service, 1, false);
    let response = service.prepare(&text(&p)).unwrap();
    assert!(service.state.frame.is_none());
    assert_eq!(service.state.workspace.footprint().live, 0);
    assert_eq!(budget.usage().reserved, before);
    assert_eq!(pcnd::write(&service.state.base).unwrap(), bytes);
    let q = service
        .state
        .profile
        .workload
        .parse_quote(field(&response, "workQuote").unwrap())
        .unwrap();
    assert_eq!(q.noise, 2 * 2 * 4096);
    assert!(q.base > 0);
}
#[test]
fn low_base_and_low_noise_refuse_before_execution_without_cross_component_credit() {
    for (d, limits) in [
        (
            doc(NoiseDistribution::Uniform, Some(1)),
            NoiseLimits {
                bytes: 64 << 20,
                base_work: 1,
                noise_work: 1 << 35,
            },
        ),
        (
            doc(NoiseDistribution::Uniform, None),
            NoiseLimits {
                bytes: 64 << 20,
                base_work: 268_435_456,
                noise_work: 1,
            },
        ),
    ] {
        let mut s = open(&d, limits);
        let p = request(&s, 1, true);
        assert_eq!(
            s.prepare(&text(&p)).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert!(s.state.frame.is_none());
        assert!(s.state.candidate.is_none());
        assert_eq!(s.state.workspace.footprint().live, 0);
    }
}
#[test]
fn installed_legacy_caller_never_constructs_a_bigger_noise_fallback() {
    let d = doc(NoiseDistribution::Uniform, None);
    let mut s = open(&d, NoiseJobPolicy::BrowserV1.max_limits());
    let p = request(&s, 1, true);
    let b = SharedBudget::new(1 << 20);
    let legacy = paintcraft_render::spatial::WorkspaceAdmission::new(
        b.scope(AllocationClass::SharedBudget),
        1 << 20,
        268_435_456,
    );
    assert_eq!(
        paintcraft_render::spatial::with_workspace(legacy.clone(), || s.prepare(&text(&p)))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(legacy.work_used(), 0);
    assert_eq!(s.state.workspace.footprint().live, 0);
    assert!(s.state.frame.is_none());
}
#[test]
fn owner_refusal_preserves_accepted_frame_but_accepted_bad_capability_retires_it() {
    let d = doc(NoiseDistribution::Uniform, None);
    let mut s = open(&d, NoiseJobPolicy::BrowserV1.max_limits());
    let p = request(&s, 1, true);
    s.prepare(&text(&p)).unwrap();
    let generation = s.state.generation;
    let mut wrong = request(&s, 2, true);
    set(&mut wrong, "sourceSha256", Value::str("0".repeat(64)));
    assert_eq!(
        s.prepare(&text(&wrong)).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(s.state.generation, generation);
    assert!(s.state.frame.is_some());
    let mut bad = request(&s, 2, true);
    let mut fact = field(&bad, "candidateRequirements").unwrap().clone();
    set(&mut fact, "capabilitySha256", Value::str("0".repeat(64)));
    set(&mut bad, "candidateRequirements", fact);
    assert_eq!(
        s.prepare(&text(&bad)).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(s.state.generation, Some(2));
    assert!(s.state.frame.is_none());
    assert!(s.state.candidate.is_none());
    assert_eq!(s.state.workspace.footprint().live, 0);
}
#[test]
fn step_cancel_retires_private_storage_and_stale_cancel_cannot_retire_new_generation() {
    let d = doc(NoiseDistribution::Gaussian, None);
    let mut s = open(&d, NoiseJobPolicy::BrowserV1.max_limits());
    let p = request(&s, 1, true);
    s.prepare(&text(&p)).unwrap();
    let error = s
        .step_checked(&|| {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "injected step cancellation",
            ))
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert!(s.state.frame.is_none());
    assert_eq!(s.state.workspace.footprint().live, 0);
    assert_eq!(
        s.prepare(&text(&p)).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    let p = request(&s, 2, true);
    s.prepare(&text(&p)).unwrap();
    assert_eq!(s.cancel(1).unwrap_err().code, ErrorCode::ConflictStaleBase);
    assert!(s.state.frame.is_some());
    s.cancel(2).unwrap();
    assert!(s.state.frame.is_none());
    assert_eq!(s.state.workspace.footprint().live, 0);
}
#[test]
fn escaped_small_frame_blocks_unbounded_retention_then_releases_for_next_generation() {
    let d = doc(NoiseDistribution::Uniform, None);
    let mut s = open(&d, NoiseJobPolicy::BrowserV1.max_limits());
    let p = request(&s, 1, true);
    s.prepare(&text(&p)).unwrap();
    finish(&mut s);
    let packet = s.take_frame().unwrap();
    for generation in 2..10 {
        let p = request(&s, generation, true);
        assert_eq!(
            s.prepare(&text(&p)).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert!(s.state.frame.is_none());
        assert_eq!(s.state.generation, Some(1));
    }
    drop(packet);
    let p = request(&s, 2, true);
    s.prepare(&text(&p)).unwrap();
    finish(&mut s);
    let second = s.take_frame().unwrap();
    assert_eq!(second.pixels(), [255, 211, 0, 255, 51, 0, 97, 255]);
}
#[test]
fn selected_source6_slice_admits_noise_halo_that_old_total_slice_would_refuse() {
    let mut d = paintcraft_document::new_document(
        66,
        66,
        Some(paintcraft_core::types::Rgba([64, 16, 128, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let base = doc(NoiseDistribution::Gaussian, Some(32));
    d.layers.get_mut(&d.roots[0]).unwrap().filters = base.layers[&base.roots[0]].filters.clone();
    let (bytes, p) = source(&d, NoiseJobPolicy::BrowserV1.max_limits());
    let old = SlicePolicy::for_qualification(64, 8_388_608, 8_388_608).unwrap();
    let mut denied = NoiseService::open(&bytes, &text(&p), old).unwrap();
    let mut request = request(&denied, 1, false);
    let view = Value::object([
        (
            "region",
            Value::Array(vec![
                Value::Int(32),
                Value::Int(32),
                Value::Int(1),
                Value::Int(1),
            ]),
        ),
        ("level", Value::Int(0)),
    ]);
    set(&mut request, "view", view.clone());
    assert_eq!(
        denied.prepare(&text(&request)).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let mut selected =
        NoiseService::open(&bytes, &text(&p), SlicePolicy::selected_qualification()).unwrap();
    set(&mut request, "view", view);
    let reply = selected.prepare(&text(&request)).unwrap();
    let q = selected
        .state
        .profile
        .workload
        .parse_quote(field(&reply, "workQuote").unwrap())
        .unwrap();
    assert_eq!(q.noise, 65 * 65 * 4096);
    assert!(q.base <= 8_388_608);
    assert!(q.noise <= 134_217_728);
}

#[test]
fn full_1080p_noise_before_box32_quotes_exact_64_patch_halos_within_both_job_components() {
    let mut d = paintcraft_document::new_document(
        1920,
        1080,
        Some(paintcraft_core::types::Rgba([64, 16, 128, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let base = doc(NoiseDistribution::Gaussian, Some(32));
    d.layers.get_mut(&d.roots[0]).unwrap().filters = base.layers[&base.roots[0]].filters.clone();
    let mut s = open(&d, NoiseJobPolicy::BrowserV1.max_limits());
    let p = request(&s, 1, false);
    let response = s.prepare(&text(&p)).unwrap();
    let q = s
        .state
        .profile
        .workload
        .parse_quote(field(&response, "workQuote").unwrap())
        .unwrap();
    // Independent finite rectangle accounting: x widths96+28*128+96=3776;
    // y heights96+15*128+88=2104. Gaussian prefix runs once spatially.
    assert_eq!(q.noise, 3776u64 * 2104 * 4096);
    assert_eq!(q.noise, 32_541_507_584);
    assert!(q.base <= 268_435_456);
    assert!(q.noise <= 1 << 35);
    let view = DisplayRequest::new(&d, d.canvas_rect(), 0).unwrap();
    let quoted = quote::frame_quote(&d, view, s.state.profile, s.state.policy, &|| Ok(())).unwrap();
    assert_eq!(quoted.tiles, 30 * 17);
    assert_eq!(quoted.work.noise, q.noise);
    assert!(s.state.frame.is_none());
    assert_eq!(s.state.workspace.footprint().live, 0);
}
