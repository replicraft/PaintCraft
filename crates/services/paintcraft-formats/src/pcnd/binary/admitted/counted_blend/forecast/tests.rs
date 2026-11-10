use super::super::{
    blend_counted_snapshot_control_bytes, count_portable_blend_snapshot_rooted_checked,
};
use super::*;
use paintcraft_core::types::{
    Error, ErrorCode, LayerBlendMode, Rect, Rgba,
    json::{self, Value},
    resources::{AllocationClass, BudgetScope, SharedBudget},
};
use paintcraft_document::Document;
use std::cell::{Cell, RefCell};
mod pressure;
fn cases(kind: &str) -> Vec<Value> {
    let bytes = match kind {
        "counted" => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/counted-blend-snapshot-fixtures/oracle.json"
        ))
        .as_slice(),
        _ => include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/retained-source8-layout-fixtures/oracle.json"
        ))
        .as_slice(),
    };
    json::parse(bytes)
        .unwrap()
        .get("cases")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec()
}
fn unhex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn wire(row: &Value) -> Vec<u8> {
    unhex(row.get("hex").unwrap().as_str().unwrap())
}
fn read(bytes: &[u8]) -> Document {
    let budget = SharedBudget::new(32 << 20);
    crate::pcnd::read_with_admission_checked(
        bytes,
        1 << 40,
        1 << 20,
        1024,
        crate::pcnd::ReadAdmission {
            metadata: crate::pcnd::MetadataAdmission {
                bytes: 8 << 20,
                budget: &budget,
            },
            coverage: crate::pcnd::CoverageAdmission {
                bytes: 8 << 20,
                budget: &budget,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap()
}
fn source(name: &str) -> Document {
    let row = cases("retained")
        .into_iter()
        .find(|r| r.get("name").unwrap().as_str() == Some(name))
        .unwrap();
    read(&wire(&row))
}
fn scope(b: &SharedBudget) -> BudgetScope {
    b.reserve_capacity(MAIN_MAX)
        .unwrap()
        .scope(AllocationClass::PaintMetadata)
}
fn assert_peaks(f: &BlendSourceDemand) {
    let q = crate::pcnd::portable_source8_control_bytes();
    let material = crate::pcnd::portable_source8_materializer_control_bytes().unwrap();
    let records = q.plan + 56 * f.retained.layers() + 24 * f.retained.tiles();
    assert_eq!(f.p_plan_records, records);
    assert_eq!(
        f.p_plan_census_peak,
        records + q.scan + material.retained_layout
    );
    assert_eq!(
        f.p_plan_probe_peak,
        records + appearance::transient_quote(f.retained.layers() as usize).unwrap()
    );
    assert_eq!(f.p_peak, f.p_plan_census_peak.max(f.p_plan_probe_peak));
    assert!(f.bootstrap_p_peak >= f.p_peak);
    assert_eq!(f.pointer_bytes, size_of::<usize>() as u8);
    assert!(f.stored_non_normal);
}
#[test]
fn actual_counted_wires_are_verified_forecast_and_kept_on_same_original_factory() {
    for row in cases("counted") {
        let expected = wire(&row);
        let doc = read(&expected);
        let original = doc.clone();
        let budget = SharedBudget::new(128 << 20);
        let scope = scope(&budget);
        let used = Cell::new(0u64);
        let check = || Ok(());
        let spend = |n| {
            used.set(used.get().checked_add(n).unwrap());
            Ok(())
        };
        let fact =
            count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
        let before = used.get();
        let (bytes, demand) = fact.encode_with_source_forecast_checked().unwrap();
        assert_eq!(bytes.as_bytes(), expected.as_slice());
        assert_eq!(demand.encoded_bytes, expected.len() as u64);
        use sha2::{Digest, Sha256};
        assert_eq!(
            demand.source_sha256,
            <[u8; 32]>::from(Sha256::digest(&expected))
        );
        assert_eq!(demand.document_id, *doc.document_id.as_bytes());
        assert_eq!(demand.state_id, *doc.state_id.as_bytes());
        assert_eq!(demand.canvas, [doc.canvas.0, doc.canvas.1]);
        assert_peaks(&demand);
        assert!(used.get() > before);
        assert!(doc.same_semantics(&original));
        let sizes = blend_counted_snapshot_control_bytes();
        assert_eq!(
            scope.footprint().live,
            sizes.fact + sizes.bytes_owner + expected.len() as u64
        );
        drop(fact);
        drop(scope);
        assert_eq!(budget.usage().reserved, MAIN_MAX);
        assert_eq!(bytes.as_bytes(), expected.as_slice());
        drop(bytes);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn wire_counts_keep_disabled_empty_and_duplicate_masks_icc_none_and_strings() {
    for name in [
        "empty-own",
        "nonempty-disabled-own",
        "global-two-masks",
        "mixed-duplicate-masks",
        "icc-profile-none",
        "strings",
        "one-disabled-filter",
    ] {
        let doc = source(name);
        let budget = SharedBudget::new(128 << 20);
        let scope = scope(&budget);
        let check = || Ok(());
        let spend = |_| Ok(());
        let fact =
            count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
        let (bytes, f) = fact.encode_with_source_forecast_checked().unwrap();
        assert_peaks(&f);
        let s = &f.retained;
        let expected = match name {
            "empty-own" => (1, 0, 1, 0, 0, 0),
            "nonempty-disabled-own" => (1, 0, 1, 1, 1, 1),
            "global-two-masks" => (2, 0, 2, 2, 2, 2),
            "mixed-duplicate-masks" => (1, 3, 4, 3, 3, 3),
            "one-disabled-filter" => (1, 1, 0, 0, 0, 0),
            _ => (1, 0, 0, 0, 0, 0),
        };
        assert_eq!(
            (
                s.layers(),
                s.filters(),
                s.mask_present(),
                s.mask_nonempty(),
                s.bands(),
                s.runs()
            ),
            expected
        );
        assert_eq!(f.wire_pixels, 0);
        assert_eq!(s.tiles(), 0);
        assert_eq!(s.decoded_bytes(), 0);
        assert_eq!(s.q_fixed(), 0);
        if name == "icc-profile-none" {
            assert_eq!(s.attachment_count(), 1);
            assert_eq!(s.profile_bytes(), 0);
            assert!(s.m_icc() > 0 && s.icc_retained() > 0);
        }
        if name == "strings" {
            assert_eq!(s.string_bytes(), 9);
        }
        drop(bytes);
        drop(fact);
        assert_eq!(scope.footprint().live, 0);
    }
}
#[test]
fn omitted_default_tile_uses_resident_bootstrap_but_actual_wire_record_demand() {
    let doc = source("default-raw");
    let budget = SharedBudget::new(128 << 20);
    let scope = scope(&budget);
    assert_eq!(doc.layers[&doc.roots[0]].raster.resident_tiles(), 1);
    let check = || Ok(());
    let spend = |_| Ok(());
    let fact = count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
    let (bytes, f) = fact.encode_with_source_forecast_checked().unwrap();
    assert_eq!(
        f.record_upper,
        BlendPlanRecordUpperBounds {
            layers: 1,
            tiles: 1
        }
    );
    assert_eq!(f.wire_pixels, 0);
    assert_eq!(f.retained.tiles(), 0);
    assert_eq!(f.retained.scratch_bytes(), 0);
    assert_eq!(f.retained.decoded_bytes(), 0);
    assert_eq!(f.retained.q_fixed(), 0);
    assert_peaks(&f);
    assert!(f.bootstrap_p_peak > f.p_peak);
    drop(bytes);
    drop(fact);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn valid_run_lz4_tile_reports_actual_r_plus_d_without_decoding_in_forecast() {
    use paintcraft_raster::{Admission, BlendMode};
    let mut doc = paintcraft_document::new_document(
        256,
        256,
        Some(Rgba::TRANSPARENT),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.blend_mode = LayerBlendMode::Multiply;
    let mut admission = Admission::new(1 << 20);
    for (x, color) in [(0, Rgba([255, 0, 0, 255])), (128, Rgba([0, 0, 255, 255]))] {
        layer
            .raster
            .fill_rect(
                &Rect::new(x, 0, 128, 256).unwrap(),
                color,
                BlendMode::Replace,
                &mut admission,
            )
            .unwrap();
    }
    let budget = SharedBudget::new(128 << 20);
    let scope = scope(&budget);
    let check = || Ok(());
    let spend = |_| Ok(());
    let fact = count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
    let (bytes, f) = fact.encode_with_source_forecast_checked().unwrap();
    assert_eq!(
        (f.retained.tiles(), f.retained.decoded_bytes()),
        (1, 262144)
    );
    assert_eq!(f.wire_pixels, 65536);
    assert_eq!(f.retained.scratch_bytes(), 262144 + 512 * 6);
    assert!(f.retained.m_tile() > 0 && f.retained.q_fixed() > 0);
    assert_peaks(&f);
    let checked = CheckedContainer::verify(bytes.as_bytes(), &check).unwrap();
    let plan = checked
        .portable_source8_plan_checked(Default::default(), &scope, &check, &spend)
        .unwrap();
    let block = plan.block(0, 0).unwrap();
    assert_eq!(block.encoding, 5);
    assert_eq!(
        u32::from_le_bytes(block.stored_bytes()[..4].try_into().unwrap()),
        3072
    );
    drop(plan);
    drop(bytes);
    drop(fact);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn stored_disabled_noise_is_detected_even_on_hidden_zero_opacity_layer() {
    use paintcraft_core::types::filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution};
    let mut doc = source("baseline");
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.visible = false;
    layer.opacity = 0;
    layer.filters =
        paintcraft_document::FilterStack::new(vec![paintcraft_document::FilterInstance::new(
            uuid::Uuid::from_u128(99),
            false,
            Descriptor::AddNoiseRgbV1(
                AddNoiseRgbV1::new(100, 65535, false, NoiseDistribution::Gaussian).unwrap(),
            ),
        )])
        .unwrap();
    let budget = SharedBudget::new(128 << 20);
    let scope = scope(&budget);
    let check = || Ok(());
    let spend = |_| Ok(());
    let fact = count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
    let (bytes, f) = fact.encode_with_source_forecast_checked().unwrap();
    assert!(f.stored_noise && f.stored_non_normal);
    assert_eq!(f.retained.filters(), 1);
    drop(bytes);
    drop(fact);
    assert_eq!(scope.footprint().live, 0);
}

#[test]
fn partial_raw_pixel_demand_is_not_a_full_tile_staging_or_q_grant() {
    use paintcraft_raster::{Admission, BlendMode};
    let mut doc = paintcraft_document::new_document(
        1,
        1,
        Some(Rgba::TRANSPARENT),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.blend_mode = LayerBlendMode::Multiply;
    layer
        .raster
        .blend_pixel(
            0,
            0,
            Rgba([20, 60, 100, 255]),
            BlendMode::Replace,
            &mut Admission::new(1 << 20),
        )
        .unwrap();
    let budget = SharedBudget::new(128 << 20);
    let scope = scope(&budget);
    let check = || Ok(());
    let spend = |_| Ok(());
    let fact = count_portable_blend_snapshot_rooted_checked(&doc, &scope, &check, &spend).unwrap();
    let (bytes, f) = fact.encode_with_source_forecast_checked().unwrap();
    assert_eq!(f.wire_pixels, 1);
    assert_eq!(f.retained.tiles(), 1);
    assert_eq!(f.retained.decoded_bytes(), 262144);
    assert_eq!(f.retained.scratch_bytes(), 0);
    drop(bytes);
    drop(fact);
    assert_eq!(scope.footprint().live, 0);
}
