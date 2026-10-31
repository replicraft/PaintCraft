//! New source4 is separately negotiated; source2/3 stay closed.
use super::*;

fn control(doc: &Document, bytes: &[u8]) -> Value {
    let mut fields = plan(doc, bytes).as_object().unwrap().clone();
    for (name, number) in [
        ("protocol", 4),
        ("spatialCount", protocol::spatial::stored(doc)),
        ("catalogCount", protocol::spatial::stored_catalog(doc)),
        ("workspaceBytes", protocol::spatial::WORKSPACE_BYTES_MAX),
        ("workLimit", protocol::spatial::WORK_MAX),
    ] {
        fields.insert(name.into(), Value::Int(number as i64));
    }
    Value::Object(fields)
}
fn unhex(value: &str) -> Vec<u8> {
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn specimens() -> Value {
    paintcraft_core::types::json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../crates/services/paintcraft-formats/tests/pcnd-catalog-fixtures/oracle.json"
    )))
    .unwrap()
}
#[test]
fn actual_source4_frames_match_independent_layer_planes_and_preserve_saved_intent() {
    let oracle = specimens();
    let mut count = 0;
    for specimen in oracle
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s.get("accepted_planned").and_then(Value::as_bool) == Some(true))
    {
        let bytes = unhex(specimen.get("hex").unwrap().as_str().unwrap());
        let coverage = SharedBudget::new(1 << 20);
        let metadata = SharedBudget::new(1 << 20);
        let doc = pcnd::read_with_admission_checked(
            &bytes,
            u64::MAX,
            u64::MAX,
            100_000,
            pcnd::ReadAdmission {
                metadata: pcnd::MetadataAdmission {
                    bytes: 1 << 20,
                    budget: &metadata,
                },
                coverage: pcnd::CoverageAdmission {
                    bytes: 1 << 20,
                    budget: &coverage,
                    limits: Default::default(),
                },
            },
            &|| Ok(()),
        )
        .unwrap();
        let source = control(&doc, &bytes);
        let mut service = Service::open(&bytes, &text(&source)).unwrap();
        service
            .prepare(&text(&request(
                &source,
                1,
                None,
                Value::Null,
                true,
                [0, 0, 2, 1],
                0,
            )))
            .unwrap();
        let (_, actual) = finish(&mut service);
        let mut expected: Vec<u8> = specimen
            .get("evaluated_layer_rgba")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as u8)
            .collect();
        let (pixels, remainder) = expected.as_chunks_mut::<4>();
        assert!(remainder.is_empty());
        for pixel in pixels {
            if pixel[3] == 0 {
                pixel[..3].fill(0);
            }
        }
        assert_eq!(
            actual,
            expected,
            "{}",
            specimen.get("name").unwrap().as_str().unwrap()
        );
        service.retire();
        assert_eq!(pcnd::write(&service.base).unwrap(), bytes);
        count += 1;
    }
    assert_eq!(count, 8);
}

#[test]
fn closed_sources_refuse_stored_or_private_levels_and_source4_authenticates_inventory() {
    use paintcraft_core::types::filters::LevelsRgbV1;
    let doc = fixture();
    let descriptor = Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, 999, 0, 255).unwrap());
    let operation = FilterOp::Add {
        layer: LayerRef::Id(doc.roots[0]),
        alias: "levels".into(),
        descriptor,
        enabled: true,
        to_index: None,
    }
    .to_value()
    .unwrap();
    for protocol in [2, 3] {
        let bytes = pcnd::write(&doc).unwrap();
        let mut source = if protocol == 2 {
            plan(&doc, &bytes)
        } else {
            control(&doc, &bytes)
        };
        if protocol == 3 {
            let Value::Object(ref mut fields) = source else {
                unreachable!()
            };
            fields.insert("protocol".into(), Value::Int(3));
            fields.remove("catalogCount");
        }
        let mut service = Service::open(&bytes, &text(&source)).unwrap();
        assert_eq!(
            service
                .prepare(&text(&request(
                    &source,
                    1,
                    Some(doc.roots[0]),
                    operation.clone(),
                    true,
                    [0, 0, 3, 2],
                    0
                )))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert!(service.candidate.is_none());
    }
    let mut catalog = doc.clone();
    catalog.layers.get_mut(&doc.roots[0]).unwrap().filters =
        paintcraft_document::FilterStack::new(vec![paintcraft_document::FilterInstance::new(
            Uuid::from_u128(71),
            false,
            descriptor,
        )])
        .unwrap();
    let bytes = pcnd::write(&catalog).unwrap();
    assert_eq!(bytes[4], 6);
    assert_eq!(
        Service::open(&bytes, &text(&plan(&catalog, &bytes)))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let mut source = control(&catalog, &bytes);
    let Value::Object(ref mut fields) = source else {
        unreachable!()
    };
    fields.insert("catalogCount".into(), Value::Int(0));
    assert_eq!(
        Service::open(&bytes, &text(&source)).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
}

#[test]
fn source4_prepares_once_across_native_tiles_and_prices_preview_off_before_compile() {
    use paintcraft_core::types::filters::LevelsRgbV1;
    let mut doc =
        paintcraft_document::import_image(128, 1, &[64, 16, 128, 255].repeat(128), "two-tiles")
            .unwrap();
    let policy = LevelsRgbV1::new(0, 255, 200, 0, 255).unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        paintcraft_document::FilterStack::new(vec![paintcraft_document::FilterInstance::new(
            Uuid::from_u128(71),
            true,
            Descriptor::LevelsRgbV1(policy),
        )])
        .unwrap();
    let bytes = pcnd::write(&doc).unwrap();
    let source = control(&doc, &bytes);
    let mut service = Service::open(&bytes, &text(&source)).unwrap();
    service
        .prepare(&text(&request(
            &source,
            1,
            None,
            Value::Null,
            true,
            [0, 0, 128, 1],
            0,
        )))
        .unwrap();
    let compiled = service
        .frame
        .as_ref()
        .unwrap()
        .spatial
        .as_ref()
        .unwrap()
        .work_used();
    assert_eq!(
        compiled,
        paintcraft_render::prepared::quote_levels(policy).compile_work
    );
    while !service.step().unwrap() {}
    // Four native patches price extraction/candidate/mask twice for a possible
    // uniform-proof fallback, plus compose/accumulate/resolve. No recompile.
    assert_eq!(
        service
            .frame
            .as_ref()
            .unwrap()
            .spatial
            .as_ref()
            .unwrap()
            .work_used(),
        compiled + 9 * 128
    );
    let (_, pixels) = service.take_frame().unwrap();
    assert_eq!(pixels, [128, 64, 181, 255].repeat(128));
    assert_eq!(pcnd::write(&service.base).unwrap(), bytes);
    let mut denied = source.as_object().unwrap().clone();
    denied.insert("workLimit".into(), Value::Int(1));
    let denied = Value::Object(denied);
    let mut service = Service::open(&bytes, &text(&denied)).unwrap();
    assert_eq!(
        service
            .prepare(&text(&request(
                &denied,
                1,
                None,
                Value::Null,
                false,
                [0, 0, 128, 1],
                0
            )))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(service.frame.is_none());
    assert_eq!(pcnd::write(&service.base).unwrap(), bytes);
}

#[test]
fn source4_refuses_reader_known_exposure_before_adoption_including_disabled_and_zero() {
    let oracle = paintcraft_core::types::json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../crates/services/paintcraft-formats/tests/pcnd-exposure-fixtures/oracle.json"
    )))
    .unwrap();
    let mut count = 0;
    for row in oracle
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row.get("accepted").and_then(Value::as_bool) == Some(true))
    {
        count += 1;
        let bytes = unhex(row.get("hex").unwrap().as_str().unwrap());
        // This is reader-known, authenticated PCND6, not an unknown-parser failure.
        let budget = SharedBudget::new(4 << 20);
        let doc = pcnd::read_with_admission_checked(
            &bytes,
            2,
            65_536,
            1,
            pcnd::ReadAdmission {
                metadata: pcnd::MetadataAdmission {
                    bytes: 1 << 20,
                    budget: &budget,
                },
                coverage: pcnd::CoverageAdmission {
                    bytes: 1 << 20,
                    budget: &budget,
                    limits: Default::default(),
                },
            },
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(pcnd::write(&doc).unwrap(), bytes);
        let error = Service::open(&bytes, &text(&control(&doc, &bytes))).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert_eq!(
            error.message,
            "Preview source4 catalog capability is closed"
        );
    }
    assert_eq!(count, 12);
}

#[test]
fn source4_refuses_private_exposure_add_update_before_candidate_even_zero_preview_off() {
    use paintcraft_core::types::filters::ExposureSrgbV1;
    for stops in [-800, 0, 800] {
        for enabled in [false, true] {
            let mut doc = fixture();
            let layer = doc.roots[0];
            let id = Uuid::from_u128(191);
            doc.layers.get_mut(&layer).unwrap().filters =
                paintcraft_document::FilterStack::new(vec![
                    paintcraft_document::FilterInstance::new(id, enabled, Descriptor::InvertRgbV1),
                ])
                .unwrap();
            let bytes = pcnd::write(&doc).unwrap();
            assert_eq!(bytes[4], 3, "control uses only a legacy point filter");
            let source = control(&doc, &bytes);
            let descriptor = Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(stops).unwrap());
            for operation in [
                FilterOp::Add {
                    layer: LayerRef::Id(layer),
                    alias: "exposure".into(),
                    descriptor,
                    enabled,
                    to_index: None,
                },
                FilterOp::Update {
                    layer: LayerRef::Id(layer),
                    filter: FilterRef::Id(id),
                    descriptor,
                },
            ] {
                for render in [false, true] {
                    let mut service = Service::open(&bytes, &text(&source)).unwrap();
                    let error = service
                        .prepare(&text(&request(
                            &source,
                            1,
                            Some(layer),
                            operation.to_value().unwrap(),
                            render,
                            [0, 0, 3, 2],
                            0,
                        )))
                        .unwrap_err();
                    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
                    assert_eq!(
                        error.message,
                        "Preview source4 catalog capability is closed"
                    );
                    assert!(service.candidate.is_none());
                    assert!(service.frame.is_none());
                    assert_eq!(pcnd::write(&service.base).unwrap(), bytes);
                }
            }
        }
    }
}
