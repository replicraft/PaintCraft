//! Reader support does not widen the frozen source4 execution capability.
use super::*;
#[test]
fn source4_refuses_reader_known_curves_including_disabled_identity_before_adoption() {
    let oracle = paintcraft_core::types::json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../crates/services/paintcraft-formats/tests/pcnd-curves-fixtures/oracle.json"
    )))
    .unwrap();
    let mut count = 0;
    for row in oracle
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r.get("expected").unwrap().as_str() == Some("accept-after-codec-integration"))
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
        let error = Service::open(&bytes, &text(&control(&doc, &bytes))).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert_eq!(
            error.message,
            "Preview source4 catalog capability is closed"
        );
    }
    assert_eq!(count, 9);
}
#[test]
fn source4_refuses_private_curves_add_update_even_identity_without_candidate() {
    use paintcraft_core::types::filters::CurvesRgbV1;
    for outputs in [[0, 64, 128, 192, 255], [255, 0, 255, 0, 255]] {
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
        let source = control(&doc, &bytes);
        let descriptor = Descriptor::CurvesRgbV1(CurvesRgbV1::new(outputs));
        for operation in [
            FilterOp::Add {
                layer: LayerRef::Id(layer),
                alias: "curves".into(),
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
