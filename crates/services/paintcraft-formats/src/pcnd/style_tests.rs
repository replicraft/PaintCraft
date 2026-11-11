//! Literal project expectations predate the PCND9 implementation.
use super::*;
fn specimen(name: &str) -> Vec<u8> {
    let o = paintcraft_core::types::json::parse(include_bytes!(
        "../../tests/color-overlay-project-fixtures/wire.json"
    ))
    .unwrap();
    let row = o
        .get("scenes")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("name").and_then(|v| v.as_str()) == Some(name))
        .unwrap();
    let text = row.get("hex").unwrap().as_str().unwrap();
    let b: Vec<u8> = (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect();
    assert_eq!(
        paintcraft_core::types::policy::hex(&Sha256::digest(&b)),
        row.get("sha256").unwrap().as_str().unwrap()
    );
    b
}
fn adopt(bytes: &[u8]) -> Result<Document> {
    let original = SharedBudget::new(1 << 20);
    read_with_admission_checked(
        bytes,
        16,
        64,
        2,
        ReadAdmission {
            metadata: MetadataAdmission {
                bytes: 0,
                budget: &original,
            },
            coverage: CoverageAdmission {
                bytes: 1 << 20,
                budget: &original,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
}
fn altered(bytes: &[u8], edit: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut body = bytes[..bytes.len() - 32].to_vec();
    edit(&mut body);
    let digest = Sha256::digest(&body);
    body.extend_from_slice(&digest);
    body
}
#[test]
fn literal_pcnd9_single_disabled_and_full_appearance_roundtrip_exact_bytes() {
    for name in [
        "single",
        "disabled-opacity-zero",
        "filter-styles-mask-blend",
    ] {
        let bytes = specimen(name);
        let doc = adopt(&bytes).unwrap();
        assert!(doc.has_styles());
        assert_eq!(write_compact(&doc).unwrap(), bytes);
        let h = probe(&bytes).unwrap();
        assert_eq!(h.document_schema, "paintcraft.document/v9");
        assert!(
            h.required_features
                .iter()
                .any(|s| s == LAYER_STYLES_FEATURE)
        );
        h.require_supported().unwrap();
        let again = adopt(&write_compact(&doc).unwrap()).unwrap();
        assert!(doc.same_semantics(&again));
        assert_eq!(doc.state_id, again.state_id);
    }
    let disabled = adopt(&specimen("disabled-opacity-zero")).unwrap();
    let style = disabled.layers_in_order().next().unwrap().styles.as_slice()[0];
    assert!(!style.enabled());
    assert_eq!(
        style.descriptor().semantic_parameter_bytes(),
        [17, 93, 201, 0, 2]
    );
}
#[test]
fn empty_styles_select_legacy_minimum_and_style_header_cannot_lie() {
    let mut doc = adopt(&specimen("single")).unwrap();
    for layer in doc.layers.values_mut() {
        layer.styles = paintcraft_document::StyleStack::empty();
    }
    let old = write_compact(&doc).unwrap();
    assert_eq!(old[4], 1);
    let old_doc = read(&old).unwrap();
    assert!(old_doc.same_semantics(&doc));
    let again = write_compact(&old_doc).unwrap();
    assert_eq!(again, old);
    let mut h = probe(&specimen("single")).unwrap();
    h.required_features.retain(|s| s != LAYER_STYLES_FEATURE);
    assert_eq!(
        h.require_supported().unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let mut old_h = probe(&old).unwrap();
    old_h.required_features.push(LAYER_STYLES_FEATURE.into());
    assert_eq!(
        old_h.require_supported().unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn authenticated_unknown_style_tags_modes_nil_duplicates_and_counts_refuse() {
    let bytes = specimen("single");
    let id = bytes.windows(16).position(|b| b == [0x14; 16]).unwrap();
    for (offset, value, code) in [
        (16, 2, ErrorCode::CodecFailed),
        (17, 2, ErrorCode::CapabilityUnsupported),
        (22, 3, ErrorCode::CodecFailed),
    ] {
        assert_eq!(
            adopt(&altered(&bytes, |b| b[id + offset] = value))
                .unwrap_err()
                .code,
            code
        );
    }
    assert_eq!(
        adopt(&altered(&bytes, |b| b[id..id + 16].fill(0)))
            .unwrap_err()
            .code,
        ErrorCode::CodecFailed
    );
    assert_eq!(
        adopt(&altered(&bytes, |b| b[id - 2..id]
            .copy_from_slice(&257u16.to_be_bytes())))
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    let pair = specimen("filter-styles-mask-blend");
    let first = pair.windows(16).position(|b| b == [0x14; 16]).unwrap();
    let second = pair.windows(16).position(|b| b == [0x15; 16]).unwrap();
    assert_eq!(
        adopt(&altered(&pair, |b| {
            let value = b[first..first + 16].to_vec();
            b[second..second + 16].copy_from_slice(&value);
        }))
        .unwrap_err()
        .code,
        ErrorCode::CodecFailed
    );
    let mut corrupt = bytes.clone();
    corrupt[id] ^= 1;
    assert!(adopt(&corrupt).unwrap_err().message.contains("SHA-256"));
}
#[test]
fn old_appearance_writers_refuse_styles_through_paid_root_validation() {
    use paintcraft_core::types::resources::AllocationClass;
    let original = SharedBudget::new(1 << 20);
    let scope = original
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    for name in [
        "single",
        "disabled-opacity-zero",
        "filter-styles-mask-blend",
    ] {
        let doc = adopt(&specimen(name)).unwrap();
        let spent = std::cell::Cell::new(17u64);
        let spend = |n| {
            spent.set(spent.get().checked_add(n).unwrap());
            Ok(())
        };
        let error = count_portable_blend_snapshot_rooted_checked(&doc, &scope, &|| Ok(()), &spend)
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert!(spent.get() > 17);
        assert_eq!(scope.footprint().live, 0);
    }
}
