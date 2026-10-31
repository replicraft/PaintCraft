use super::*;
use paintcraft_core::types::{filters::ExposureSrgbV1, json};
#[test]
fn independent_required_subset_hashes_and_canonical_order_match_contract() {
    let oracle = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/prepared-capability-fixtures/oracle.json"
    )))
    .unwrap();
    let rows = oracle.get("rows").unwrap().as_array().unwrap();
    assert_eq!(rows.len(), 5);
    for row in rows {
        let mut required = Requirements::default();
        for name in row
            .get("profiles")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .rev()
        {
            required
                .insert(name.as_str().unwrap(), "params-v1")
                .unwrap();
            required
                .insert(name.as_str().unwrap(), "params-v1")
                .unwrap();
        }
        let (a, b) = required.digests(&|| Ok(())).unwrap();
        assert_eq!(
            paintcraft_document::hex(&a),
            row.get("required_sha256").unwrap().as_str().unwrap()
        );
        assert_eq!(
            paintcraft_document::hex(&b),
            row.get("resolution_sha256").unwrap().as_str().unwrap()
        );
    }
}
#[test]
fn every_registration_has_typed_decoder_execution_and_pricing_and_unknowns_refuse() {
    assert_eq!(registrations().len(), 10);
    assert!(
        descriptor_requires_generic(Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(0).unwrap()))
            .unwrap()
    );
    assert!(
        !descriptor_requires_generic(Descriptor::LevelsRgbV1(
            paintcraft_core::types::filters::LevelsRgbV1::default()
        ))
        .unwrap()
    );

    let exposure = resolve("exposure-srgb-v1", "params-v1").unwrap();
    assert_eq!(exposure.execution(), Execution::PreparedExposure);
    assert_eq!(
        exposure.resource_contract(),
        ("prepared-exposure-rgba8-v1", 1)
    );
    for n in [-800i16, 0, 800] {
        assert_eq!(
            exposure.decode(&n.to_be_bytes()).unwrap(),
            Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(n).unwrap())
        );
    }
    for bytes in [&[][..], &[0][..], &[0, 0, 0][..], &801i16.to_be_bytes()[..]] {
        assert!(exposure.decode(bytes).is_err());
    }
    for (profile, codec) in [
        ("reader-only-v1", "params-v1"),
        ("exposure-srgb-v1", "params-v2"),
    ] {
        assert_eq!(
            resolve(profile, codec).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
    }
    for entry in registrations() {
        assert!(!entry.resource_contract().0.is_empty());
        assert_eq!(entry.resource_contract().1, 1);
    }
}
#[test]
fn hashes_depend_on_required_resolution_only_and_union_retains_source_intent() {
    let mut source = Requirements::default();
    source.insert("invert-rgb-v1", "params-v1").unwrap();
    let before = source.digests(&|| Ok(())).unwrap();
    let mut candidate = Requirements::default();
    candidate.insert("exposure-srgb-v1", "params-v1").unwrap();
    let union = source.union(&candidate, &|| Ok(())).unwrap();
    assert_eq!(union.len(), 2);
    assert_eq!(source.digests(&|| Ok(())).unwrap(), before);
    let mut changed = source.clone();
    static REVISED: Registration = Registration {
        profile: "invert-rgb-v1",
        codec: "params-v1",
        decode: decoders::invert,
        execution: Execution::DirectPoint,
        resource: "direct-rgba8-point-v1",
        revision: 2,
    };
    changed.entries[0] = Some(&REVISED);
    let after = changed.digests(&|| Ok(())).unwrap();
    assert_eq!(before.0, after.0);
    assert_ne!(before.1, after.1);
    assert_eq!(
        source
            .union(&Requirements::default(), &|| Ok(()))
            .unwrap()
            .digests(&|| Ok(()))
            .unwrap(),
        before
    );
}

#[test]
fn curves_registration_is_explicit_priced_and_exact_five_bytes() {
    use paintcraft_core::types::filters::CurvesRgbV1;
    let entry = resolve("curves-rgb-v1", "params-v1").unwrap();
    assert_eq!(entry.execution(), Execution::PreparedCurves);
    assert_eq!(entry.resource_contract(), ("prepared-curves-rgba8-v1", 1));
    for outputs in [[0, 64, 128, 192, 255], [255, 0, 255, 0, 255], [0; 5]] {
        let descriptor = Descriptor::CurvesRgbV1(CurvesRgbV1::new(outputs));
        assert_eq!(entry.decode(&outputs).unwrap(), descriptor);
        assert!(descriptor_requires_generic(descriptor).unwrap());
    }
    for bad in [&[][..], &[0; 4][..], &[0; 6][..]] {
        assert_eq!(entry.decode(bad).unwrap_err().code, ErrorCode::CodecFailed);
    }
    assert_eq!(
        resolve("curves-rgb-v1", "params-v2").unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(registrations()[9].profile(), "curves-rgb-v1");
}
