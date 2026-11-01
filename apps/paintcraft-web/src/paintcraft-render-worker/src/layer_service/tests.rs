//! Authored actual API controls, unexecuted until Root's serialized gate.
use super::*;
use paintcraft_core::types::{
    Rect, Rgba,
    filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution},
    ids::deterministic_id,
};
use paintcraft_document::{FilterInstance, FilterMask, FilterStack, LayerMask};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use std::cell::Cell;
fn fixture(value: u8, enabled: bool) -> Document {
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([20, 115, 200, 128])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.mask = Some(LayerMask::new(
        enabled,
        A8Mask::uniform(
            Rect::new(0, 0, 2, 1).unwrap(),
            value,
            &SharedBudget::new(1 << 20),
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap(),
    ));
    layer.opacity = 128;
    doc
}
fn admission(noise: bool) -> Admission7 {
    Admission7 {
        mode: if noise { Mode7::Noise } else { Mode7::Base },
        workspace_bytes: 64 << 20,
        base_work: 268_435_456,
        noise_work: if noise { 1 << 35 } else { 0 },
    }
}
fn set(v: &mut Value, k: &str, value: Value) {
    let Value::Object(o) = v else {
        panic!("object")
    };
    o.insert(k.into(), value);
}
fn source(doc: &Document, a: Admission7) -> (Vec<u8>, Value) {
    let bytes = pcnd::write(doc).unwrap();
    let b = SharedBudget::new(32 << 20);
    let scope = b.scope(AllocationClass::SharedBudget);
    let (verified, sha) =
        pcnd::CheckedContainer::verify_with_source_hash(&bytes, &|| Ok(())).unwrap();
    let probe = verified
        .probe_layer_appearance_checked(
            pcnd::RequirementsLimits {
                layers: doc.layers.len(),
                tiles: 8,
                metadata_bytes: 8 << 20,
                coverage: Limits::default(),
                work: 268_435_456,
            },
            &scope,
            &|| Ok(()),
            &|_| Ok(()),
        )
        .unwrap();
    let inv = probe.inventory();
    let filters = inv.filter_coverage().unwrap();
    let mut full = Fact7::capture(probe.requirements(), a, inv, &|| Ok(()))
        .unwrap()
        .value();
    set(
        &mut full,
        "pairs",
        Value::Array(
            probe
                .requirements()
                .requirements()
                .pairs()
                .map(|(p, c)| {
                    Value::object([("profile_id", Value::str(p)), ("codec_id", Value::str(c))])
                })
                .collect(),
        ),
    );
    let digest = coverage7::filter_digest_checked(doc, &scope, &|| Ok(()), &|_| Ok(())).unwrap();
    let value = Value::object([
        ("protocol", Value::Int(7)),
        ("registryContract", Value::Int(3)),
        (
            "owner",
            Value::object([
                (
                    "session",
                    Value::str(deterministic_id(&["Source7 test owner"]).to_string()),
                ),
                ("state", Value::str(doc.state_id.to_string())),
                ("epoch", Value::str("0")),
                ("frontend", Value::Int(3)),
            ]),
        ),
        (
            "draft",
            Value::str(deterministic_id(&["Source7 test draft"]).to_string()),
        ),
        ("document", Value::str(doc.document_id.to_string())),
        ("sourceSha256", Value::str(hex(&sha))),
        (
            "size",
            Value::Array(vec![
                Value::Int(i64::from(doc.canvas.0)),
                Value::Int(i64::from(doc.canvas.1)),
            ]),
        ),
        ("layers", Value::Int(doc.layers.len() as i64)),
        (
            "residentBytes",
            Value::Int(
                doc.layers
                    .values()
                    .map(|l| {
                        l.raster.resident_tiles() as i64
                            * paintcraft_raster::RasterLayer::TILE_BYTES as i64
                    })
                    .sum(),
            ),
        ),
        ("metadataBytes", Value::Int(0)),
        ("metadataCharge", Value::Int(0)),
        ("filterDigest", Value::str(hex(&digest))),
        ("maskCount", Value::Int(filters.masks as i64)),
        ("maskBands", Value::Int(filters.bands as i64)),
        ("maskRuns", Value::Int(filters.runs as i64)),
        ("maskDecodeBytes", Value::Int(filters.decode_bytes as i64)),
        ("sourceRequirements", full),
        ("appearance", metadata::appearance_registration().value()),
        ("admission", a.value().unwrap()),
        ("slice", a.slice_value()),
    ]);
    (bytes, value)
}
fn open(doc: &Document, noise: bool) -> LayerAppearanceService {
    let (bytes, control) = source(doc, admission(noise));
    LayerAppearanceService::open(&bytes, &text(&control)).unwrap()
}
fn request(s: &LayerAppearanceService, generation: u64, render: bool, operation: Value) -> Value {
    let state = &s.state;
    let b = SharedBudget::new(32 << 20);
    let scope = b.scope(AllocationClass::SharedBudget);
    let mut provenance = CandidateProvenance::Unchanged;
    let candidate = if operation == Value::Null {
        state.base.clone()
    } else {
        let (batch, _) = begin::batch(state, &operation, state.base.roots[0], &|| Ok(())).unwrap();
        let stage = paintcraft_editor::execute::execute_with_admission_checked(
            &state.base,
            paintcraft_raster::Selection::Inactive,
            &batch,
            paintcraft_raster::Admission::new(0),
            &|| Ok(()),
        )
        .unwrap();
        if let Op::Filter(FilterOp::Duplicate {
            filter: FilterRef::Id(id),
            alias,
            ..
        }) = &batch.operations[0]
        {
            provenance = CandidateProvenance::Duplicate {
                layer: state.base.roots[0],
                source_filter: *id,
                new_filter: stage.filter_aliases[alias].1,
            };
        }
        stage.doc
    };
    let numerical = Requirements::capture(&candidate, &|| Ok(())).unwrap();
    let required =
        Requirements7::new(numerical, metadata::appearance_registration(), &|| Ok(())).unwrap();
    let inventory = state
        .adopted
        .candidate_inventory_checked(
            &state.base,
            &candidate,
            provenance,
            &scope,
            &|| Ok(()),
            &|_| Ok(()),
        )
        .unwrap();
    let fact = Fact7::capture(&required, state.profile.admission, inventory, &|| Ok(())).unwrap();
    let union = state
        .adopted
        .requirements()
        .union(&required, &|| Ok(()))
        .unwrap();
    let union = Union7::capture(
        &union,
        state.profile.admission,
        state.profile.source.source_coverage,
        &|| Ok(()),
    )
    .unwrap();
    Value::object([
        ("protocol", Value::Int(7)),
        ("registryContract", Value::Int(3)),
        (
            "owner",
            field(&state.source.value, "owner").unwrap().clone(),
        ),
        (
            "draft",
            field(&state.source.value, "draft").unwrap().clone(),
        ),
        (
            "sourceSha256",
            field(&state.source.value, "sourceSha256").unwrap().clone(),
        ),
        ("generation", Value::str(generation.to_string())),
        (
            "target",
            if operation == Value::Null {
                Value::Null
            } else {
                Value::str(state.base.roots[0].to_string())
            },
        ),
        ("operation", operation),
        (
            "view",
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
        ),
        ("render", Value::Bool(render)),
        ("candidateRequirements", fact.value()),
        ("unionRequirements", union.value()),
        ("admissionSha256", Value::str(hex(&state.admission_sha256))),
    ])
}
fn finish(s: &mut LayerAppearanceService) {
    for _ in 0..100 {
        if s.step().unwrap() {
            return;
        }
    }
    panic!("finite frame");
}
#[test]
fn filterless_mask_alpha_order_actual_output_and_one_escape_bound() {
    let mut s = open(&fixture(128, true), false);
    assert_eq!(s.state.profile.source.count, 0);
    assert_eq!(s.state.profile.source.own.stored, 1);
    let bytes_before = pcnd::write(&s.state.base).unwrap();
    let r = request(&s, 1, true, Value::Null);
    s.prepare(&text(&r)).unwrap();
    finish(&mut s);
    let packet = s.take_frame().unwrap();
    assert_eq!(packet.pixels(), [20, 115, 200, 32, 20, 115, 200, 32]);
    let next = request(&s, 2, true, Value::Null);
    assert_eq!(
        s.prepare(&text(&next)).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    drop(packet);
    s.prepare(&text(&next)).unwrap();
    assert_eq!(pcnd::write(&s.state.base).unwrap(), bytes_before);
}
#[test]
fn disabled_and_empty_intents_remain_required_but_quoteonly_does_not_compile_or_allocate_frame() {
    for (value, enabled, expected) in [
        (255, false, vec![20, 115, 200, 64, 20, 115, 200, 64]),
        (0, true, vec![0; 8]),
    ] {
        let mut s = open(&fixture(value, enabled), false);
        let old = s.state.job.workspace().work_used();
        let live = s.state.job.workspace().scope().footprint().live;
        let r = request(&s, 1, false, Value::Null);
        s.prepare(&text(&r)).unwrap();
        assert!(s.state.frame.is_none());
        assert!(!s.state.job.workspace().has_prepared_programs());
        assert_eq!(s.state.job.workspace().scope().footprint().live, live);
        assert!(s.state.job.workspace().work_used() > old);
        let r = request(&s, 2, true, Value::Null);
        s.prepare(&text(&r)).unwrap();
        finish(&mut s);
        assert_eq!(s.take_frame().unwrap().pixels(), expected);
    }
}
#[test]
fn same_open_counter_survives_cancel_and_denial_without_new_allowance() {
    let mut s = open(&fixture(128, true), false);
    let r = request(&s, 1, true, Value::Null);
    s.prepare(&text(&r)).unwrap();
    let used = s.state.job.workspace().work_used();
    s.cancel(1).unwrap();
    assert_eq!(s.state.job.workspace().work_used(), used);
    let parent = s.state.job.workspace().clone();
    let remaining = parent.work_limit() - parent.work_used();
    parent.reserve_work(remaining).unwrap();
    let r = request(&s, 2, false, Value::Null);
    assert_eq!(
        s.prepare(&text(&r)).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(parent.work_used(), parent.work_limit());
    assert_eq!(s.state.job.workspace().work_used(), parent.work_limit());
}
#[test]
fn real_cancellation_retires_private_output_and_corrupted_source_never_adopts() {
    let d = fixture(128, true);
    let (mut bytes, control) = source(&d, admission(false));
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    assert!(LayerAppearanceService::open(&bytes, &text(&control)).is_err());
    let mut s = open(&d, false);
    let r = request(&s, 1, true, Value::Null);
    s.prepare(&text(&r)).unwrap();
    let e = s
        .step_checked(&|| Err(Error::new(ErrorCode::OperationCancelled, "test")))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::OperationCancelled);
    assert!(s.state.frame.is_none());
    assert!(s.take_frame().is_err());
    assert!(s.state.candidate.is_none());
}
#[test]
fn duplicate_remove_disabled_filter_inventory_is_distinct_from_union() {
    let mut d = fixture(128, true);
    let l = d.layers.get_mut(&d.roots[0]).unwrap();
    let id = deterministic_id(&["Source7 disabled filter"]);
    let coverage = l.mask.as_ref().unwrap().coverage().clone();
    l.filters = FilterStack::new(vec![
        FilterInstance::new(id, false, Descriptor::InvertRgbV1)
            .with_mask(Some(FilterMask::new(true, coverage))),
    ])
    .unwrap();
    let mut s = open(&d, false);
    let source = s.state.profile.source;
    let mut old_union = None;
    for (i, name) in ["filter.duplicate", "filter.remove", "filter.set_enabled"]
        .iter()
        .enumerate()
    {
        let mut op = Value::object([
            ("op", Value::str(*name)),
            (
                "layer_ref",
                Value::object([("id", Value::str(d.roots[0].to_string()))]),
            ),
            (
                "filter_ref",
                Value::object([("id", Value::str(id.to_string()))]),
            ),
        ]);
        if *name == "filter.duplicate" {
            set(&mut op, "ref", Value::str("duplicate"));
        }
        if *name == "filter.set_enabled" {
            set(&mut op, "enabled", Value::Bool(true));
        }
        let r = request(&s, i as u64 + 1, false, op);
        let result = s.prepare(&text(&r)).unwrap();
        let fact = field(result, "candidateRequirements").unwrap();
        assert_ne!(
            field(fact, "maskInventorySha256").unwrap(),
            &Value::str(hex(&source.mask_inventory))
        );
        assert_eq!(field(fact, "layerMasks").unwrap(), &source.own.value());
        let union = field(result, "unionRequirements").unwrap();
        if let Some(old) = &old_union {
            assert_eq!(union, old);
        } else {
            old_union = Some(union.clone());
        }
    }
}
#[test]
fn stored_disabled_noise_requires_paired_open_and_unused_prospective_pair_is_allowed() {
    let mut d = fixture(128, true);
    let layer = d.layers.get_mut(&d.roots[0]).unwrap();
    layer.filters = FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["Source7 disabled Noise"]),
        false,
        Descriptor::AddNoiseRgbV1(
            AddNoiseRgbV1::new(0, 65535, false, NoiseDistribution::Gaussian).unwrap(),
        ),
    )])
    .unwrap();
    let (bytes, mut c) = source(&d, admission(true));
    set(&mut c, "admission", admission(false).value().unwrap());
    set(&mut c, "slice", admission(false).slice_value());
    assert_eq!(
        LayerAppearanceService::open(&bytes, &text(&c))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let mut s = open(&d, true);
    assert!(matches!(s.state.job, OpenJob::Noise(_)));
    let r = request(&s, 1, false, Value::Null);
    s.prepare(&text(&r)).unwrap();
    if let OpenJob::Noise(a) = &s.state.job {
        assert_eq!(a.used().unwrap().noise, 0);
    } else {
        panic!("paired");
    }
    let s = open(&fixture(128, true), true);
    assert!(matches!(s.state.job, OpenJob::Noise(_)));
}
#[test]
fn actual_ready_layout_and_callback_minima_are_not_guessed_abi_literals() {
    let q = LayerAppearanceService::control_quote().unwrap();
    println!(
        "Source7 ptr={} state={} service={} frame={} retained={} parser={} transient1={}",
        std::mem::size_of::<usize>(),
        std::mem::size_of::<State>(),
        std::mem::size_of::<LayerAppearanceService>(),
        q.frame_bytes,
        q.retained_bytes,
        wire::CONTROL_BYTES,
        coverage7::transient_quote(1).unwrap()
    );
    for noise in [false, true] {
        let ready = LayerAppearanceService::ready(noise).unwrap();
        assert_eq!(
            field(&ready, "controlQuote").unwrap(),
            field(field(&ready, "runtimeFacts").unwrap(), "controlQuoteBytes").unwrap()
        );
        assert_eq!(
            field(field(&ready, "runtimeFacts").unwrap(), "coverageAbi").unwrap(),
            &Value::str(CoverageAbi::current().unwrap().id())
        );
    }
    let (bytes, c) = source(&fixture(128, true), admission(false));
    let b = SharedBudget::new(1024);
    let parent = paintcraft_render::spatial::WorkspaceAdmission::new(
        b.scope(AllocationClass::SharedBudget),
        1,
        1,
    );
    let used = Cell::new(0);
    let error = paintcraft_render::spatial::with_workspace(parent.clone(), || {
        let r = LayerAppearanceService::open(&bytes, &text(&c));
        used.set(parent.work_used());
        r
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(used.get(), parent.work_used());
    assert!(parent.work_used() <= 1);
}

#[test]
fn insufficient_sha_intake_work_refuses_before_bad_trailer_and_retains_parent_spend() {
    let (mut bytes, value) = source(&fixture(128, true), admission(false));
    let wire = text(&value);
    let bootstrap = control::parse_quote(wire.len()).unwrap() + metadata::admission_work_quote();
    let limit = bootstrap + 2 * bytes.len() as u64 - 1;
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    // If authentication were reached this exact input would fail its trailer.
    assert!(pcnd::CheckedContainer::verify(&bytes, &|| Ok(())).is_err());
    let b = SharedBudget::new(64 << 20);
    let parent = paintcraft_render::spatial::WorkspaceAdmission::new(
        b.scope(AllocationClass::SharedBudget),
        64 << 20,
        limit,
    );
    let error = paintcraft_render::spatial::with_workspace(parent.clone(), || {
        LayerAppearanceService::open(&bytes, &wire)
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(parent.work_used(), bootstrap);
    parent.reserve_work(limit - bootstrap).unwrap();
    assert!(parent.reserve_work(1).is_err());
    assert_eq!(parent.work_used(), limit);
}

#[test]
fn actual_mask_work_is_not_double_debited_and_margin_stays_on_same_base() {
    let mut s = open(&fixture(128, true), false);
    let actual = paintcraft_render::layer_mask::quote_work(
        &s.state.base.layers[&s.state.base.roots[0]],
        Rect::new(0, 0, 2, 1).unwrap(),
    )
    .unwrap();
    let r = request(&s, 1, true, Value::Null);
    let binding = s.prepare(&text(&r)).unwrap();
    let q = field(binding, "workQuote").unwrap();
    assert_eq!(
        decimal(field(q, "maskWorkActual").unwrap()).unwrap(),
        actual
    );
    let upper = decimal(field(q, "maskWorkUpper").unwrap()).unwrap();
    assert!(upper >= actual);
    assert_eq!(
        decimal(field(q, "maskWorkMargin").unwrap()).unwrap(),
        upper - actual
    );
    let before = s.state.job.workspace().work_used();
    finish(&mut s);
    assert_eq!(s.state.job.workspace().work_used() - before, actual);
}
fn output(purpose: &str, selection: &[u8]) -> Value {
    Value::object([
        ("version", Value::Int(1)),
        ("purpose", Value::str(purpose)),
        ("format", Value::str("png")),
        ("quality", Value::Int(90)),
        ("matte", Value::Array(vec![Value::Int(255); 3])),
        ("frontend", Value::str("studio")),
        ("omitMetadata", Value::Bool(false)),
        ("bakeFilters", Value::Bool(false)),
        ("maskLength", Value::Int(selection.len() as i64)),
        ("maskSha256", Value::str(hex(&Sha256::digest(selection)))),
    ])
}
#[test]
fn visual_png_print_clipboard_preserve_masks_and_foreign_downloads_stay_closed() {
    for (purpose, selection, expected) in [
        ("print", vec![], vec![20, 115, 200, 32, 20, 115, 200, 32]),
        (
            "clipboard",
            vec![255, 0],
            vec![20, 115, 200, 32, 0, 0, 0, 0],
        ),
    ] {
        let mut s = open(&fixture(128, true), false);
        let before = pcnd::write(&s.state.base).unwrap();
        let r = request(&s, 1, true, Value::Null);
        assert_eq!(
            s.prepare_copy(&text(&r), &text(&output("download", &[])), &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert!(s.state.generation.is_none());
        let admitted = s
            .prepare_copy(&text(&r), &text(&output(purpose, &selection)), &|| Ok(()))
            .unwrap();
        assert_eq!(
            field(admitted, "kind").unwrap(),
            &Value::str("copy-admitted")
        );
        finish(&mut s);
        let encoded = s.encode_copy_checked(&selection, &|| Ok(())).unwrap();
        let decoded = paintcraft_formats::image::decode_with_pixel_limit(&encoded, 2).unwrap();
        assert_eq!((decoded.width, decoded.height), (2, 1));
        assert_eq!(decoded.rgba, expected);
        assert_eq!(pcnd::write(&s.state.base).unwrap(), before);
    }
}

#[test]
fn raw_mask_and_other_editor_commands_refuse_before_private_metadata_stage() {
    let mut s = open(&fixture(128, true), false);
    let before = pcnd::write(&s.state.base).unwrap();
    for (i, name) in ["filter.mask.capture", "layer.mask.set", "layer.create"]
        .iter()
        .enumerate()
    {
        let mut r = request(&s, i as u64 + 1, false, Value::Null);
        set(
            &mut r,
            "operation",
            Value::object([("op", Value::str(*name))]),
        );
        set(
            &mut r,
            "target",
            Value::str(s.state.base.roots[0].to_string()),
        );
        assert_eq!(
            s.prepare(&text(&r)).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        assert!(s.state.candidate.is_none());
        assert!(s.state.frame.is_none());
        assert_eq!(s.state.metadata_stage.footprint().live, 0);
        assert_eq!(pcnd::write(&s.state.base).unwrap(), before);
    }
}

#[test]
fn escaped_packet_keeps_budget_until_pixels_are_dropped_after_service_close() {
    let mut s = open(&fixture(128, true), false);
    let r = request(&s, 1, true, Value::Null);
    s.prepare(&text(&r)).unwrap();
    finish(&mut s);
    let packet = s.take_frame().unwrap();
    let budget = packet._output.budget();
    drop(s);
    assert!(budget.usage().reserved > 0);
    assert_eq!(packet.pixels(), [20, 115, 200, 32, 20, 115, 200, 32]);
    drop(packet);
    assert_eq!(budget.usage().reserved, 0);
}

// Explicit availability diagnostic; ignored in ordinary package suites.
#[path = "tests/view_lifetime_diagnostic.rs"]
mod view_lifetime_diagnostic;
