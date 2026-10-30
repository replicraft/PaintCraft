//! Independent bytes, appearance and policy identities for the point registry.
use paintcraft_core::types::{
    ErrorCode,
    json::{self, Value},
    resources::SharedBudget,
};
use paintcraft_formats::{export::filter_bake, pcnd};
use sha2::{Digest, Sha256};
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/pcnd-point-fixtures")
            .join(name),
    )
    .unwrap()
}
fn bytes(value: &Value) -> Vec<u8> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| u8::try_from(v.as_i64().unwrap()).unwrap())
        .collect()
}
fn read(
    data: &[u8],
    metadata: &SharedBudget,
    coverage: &SharedBudget,
) -> paintcraft_core::types::Result<paintcraft_document::Document> {
    pcnd::read_with_admission_checked(
        data,
        2,
        65_536,
        1,
        pcnd::ReadAdmission {
            metadata: pcnd::MetadataAdmission {
                bytes: 1 << 20,
                budget: metadata,
            },
            coverage: pcnd::CoverageAdmission {
                bytes: 1 << 20,
                budget: coverage,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
}

#[test]
fn independent_point_registry_bytes_rendering_and_snapshot_identities() {
    let oracle = json::parse(&fixture("oracle.json")).unwrap();
    let mut accepted = 0;
    for row in oracle.get("fixtures").unwrap().as_array().unwrap() {
        if row.get("accepted").unwrap().as_bool() != Some(true) {
            continue;
        }
        accepted += 1;
        let name = row.get("file").unwrap().as_str().unwrap();
        let raw = fixture(name);
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&raw)),
            row.get("sha256").unwrap().as_str().unwrap(),
            "independent source {name}"
        );
        let metadata = SharedBudget::new(2 << 20);
        let coverage = SharedBudget::new(2 << 20);
        let doc = read(&raw, &metadata, &coverage).unwrap();
        let layer = &doc.layers[&doc.roots[0]];
        let source: Vec<u8> = (0..2).flat_map(|x| layer.raster.get(x, 0).0).collect();
        assert_eq!(source, bytes(row.get("raw_rgba").unwrap()), "{name}");
        assert_eq!(
            doc.source_metadata.is_some(),
            row.get("attachment").unwrap().as_bool().unwrap()
        );
        let expected = row.get("filters").unwrap().as_array().unwrap();
        assert_eq!(layer.filters.len(), expected.len());
        for (filter, expected) in layer.filters.iter().zip(expected) {
            assert_eq!(
                filter.id().to_string(),
                expected.get("id").unwrap().as_str().unwrap()
            );
            assert_eq!(
                filter.enabled(),
                expected.get("enabled").unwrap().as_bool().unwrap()
            );
            assert_eq!(
                &filter.descriptor().to_value(),
                expected.get("descriptor").unwrap()
            );
            match expected.get("coverage").unwrap() {
                Value::Null => assert!(filter.mask().is_none()),
                value => {
                    let mask = filter.mask().unwrap();
                    assert!(mask.enabled());
                    assert_eq!(
                        mask.coverage().domain(),
                        paintcraft_core::types::Rect::new(0, 0, 2, 1).unwrap()
                    );
                    assert_eq!(
                        mask.coverage().coverage(0, 0),
                        value.as_i64().unwrap() as u8
                    );
                    assert_eq!(mask.coverage().coverage(1, 0), 0);
                    assert_eq!(mask.coverage().coverage(-1, 0), 0);
                }
            }
        }
        assert_eq!(
            paintcraft_document::hex(&layer.filters.digest()),
            row.get("stack_sha256").unwrap().as_str().unwrap(),
            "independent stack identity {name}"
        );
        assert_eq!(
            paintcraft_document::hex(&filter_bake::inspect(&doc, &|| Ok(())).unwrap().digest()),
            row.get("filter_inventory_sha256")
                .unwrap()
                .as_str()
                .unwrap(),
            "independent export authority {name}"
        );
        assert_eq!(pcnd::write(&doc).unwrap(), raw, "raw {name}");
        assert_eq!(pcnd::write_compact(&doc).unwrap(), raw, "compact {name}");
        pcnd::check_metadata_admission(&doc, &|| Ok(())).unwrap();
        pcnd::check_compact_metadata_admission(&doc, &|| Ok(())).unwrap();
        pcnd::probe(&raw).unwrap().require_supported().unwrap();
        assert_eq!(
            paintcraft_render::render_flat(&doc).unwrap(),
            bytes(row.get("rendered_rgba").unwrap()),
            "independent rendered plane {name}"
        );
        let copied = read(&pcnd::write_compact(&doc).unwrap(), &metadata, &coverage).unwrap();
        assert!(doc.same_semantics(&copied));
        drop(copied);
        drop(doc);
        assert_eq!(metadata.usage().reserved, 0, "metadata lifetime {name}");
        assert_eq!(coverage.usage().reserved, 0, "coverage lifetime {name}");
    }
    assert_eq!(accepted, 23);
}

#[test]
fn point_registry_rejects_invalid_scalar_and_unknown_disabled_algorithms() {
    let oracle = json::parse(&fixture("oracle.json")).unwrap();
    let mut rejected = 0;
    for row in oracle.get("fixtures").unwrap().as_array().unwrap() {
        if row.get("accepted").unwrap().as_bool() != Some(false) {
            continue;
        }
        rejected += 1;
        let name = row.get("file").unwrap().as_str().unwrap();
        let metadata = SharedBudget::new(2 << 20);
        let coverage = SharedBudget::new(2 << 20);
        let expected = match row.get("code").unwrap().as_str().unwrap() {
            "codec.failed" => ErrorCode::CodecFailed,
            "capability.unsupported" => ErrorCode::CapabilityUnsupported,
            other => panic!("Unknown independent error expectation: {other}"),
        };
        assert_eq!(
            read(&fixture(name), &metadata, &coverage).unwrap_err().code,
            expected,
            "{name}"
        );
        assert_eq!(metadata.usage().reserved, 0);
        assert_eq!(coverage.usage().reserved, 0);
    }
    assert_eq!(rejected, 6);
}
