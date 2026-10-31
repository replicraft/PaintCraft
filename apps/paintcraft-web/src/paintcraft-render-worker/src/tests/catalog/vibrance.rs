//! Reader support does not widen the frozen source4 execution capability.
use super::*;
#[test]
fn source4_refuses_reader_known_vibrance_including_disabled_identity_before_adoption() {
    let oracle = paintcraft_core::types::json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../crates/services/paintcraft-formats/tests/pcnd-vibrance-fixtures/oracle.json"
    )))
    .unwrap();
    let mut count = 0;
    for row in oracle
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r.get("accepted").unwrap().as_bool() == Some(true))
    {
        count += 1;
        let bytes = unhex(row.get("hex").unwrap().as_str().unwrap());
        let budget = SharedBudget::new(4 << 20);
        let doc = pcnd::read_with_admission_checked(
            &bytes,
            u64::MAX,
            u64::MAX,
            100_000,
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
        for protocol in [2, 3, 4] {
            let source = closed_control(&doc, &bytes, protocol);
            let error = Service::open(&bytes, &text(&source)).unwrap_err();
            assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
            if protocol == 4 {
                assert_eq!(
                    error.message,
                    "Preview source4 catalog capability is closed"
                );
            }
        }
    }
    assert_eq!(count, 10);
}
#[test]
fn closed_sources_refuse_private_vibrance_add_update_even_default_without_candidate() {
    use paintcraft_core::types::filters::VibranceRgbV1;
    for amount in [-100, 0, 100] {
        let mut doc = fixture();
        let layer = doc.roots[0];
        let id = Uuid::from_u128(191);
        doc.layers.get_mut(&layer).unwrap().filters =
            paintcraft_document::FilterStack::new(vec![paintcraft_document::FilterInstance::new(
                id,
                false,
                Descriptor::InvertRgbV1,
            )])
            .unwrap();
        let bytes = pcnd::write(&doc).unwrap();
        let descriptor = Descriptor::VibranceRgbV1(VibranceRgbV1::new(amount).unwrap());
        for protocol in [2, 3, 4] {
            let source = closed_control(&doc, &bytes, protocol);
            for operation in [
                FilterOp::Add {
                    layer: LayerRef::Id(layer),
                    alias: "vibrance".into(),
                    descriptor,
                    enabled: false,
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
                    if protocol == 4 {
                        assert_eq!(
                            error.message,
                            "Preview source4 catalog capability is closed"
                        );
                    }
                    assert!(service.candidate.is_none());
                    assert!(service.frame.is_none());
                    assert_eq!(pcnd::write(&service.base).unwrap(), bytes);
                }
            }
        }
    }
}
fn closed_control(doc: &Document, bytes: &[u8], protocol: u8) -> Value {
    if protocol == 2 {
        return plan(doc, bytes);
    }
    let mut value = control(doc, bytes);
    if protocol == 3 {
        let Value::Object(ref mut fields) = value else {
            unreachable!()
        };
        fields.insert("protocol".into(), Value::Int(3));
        fields.remove("catalogCount");
    }
    value
}
