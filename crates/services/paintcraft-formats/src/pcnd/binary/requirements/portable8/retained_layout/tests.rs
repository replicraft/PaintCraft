use super::*;
use paintcraft_core::types::{
    json,
    resources::{AllocationClass, SharedBudget},
};
mod controls;

fn rows() -> Vec<json::Value> {
    json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/retained-source8-layout-fixtures/oracle.json"
    )))
    .unwrap()
    .get("cases")
    .unwrap()
    .as_array()
    .unwrap()
    .to_vec()
}
fn bytes(row: &json::Value) -> Vec<u8> {
    row.get("hex")
        .unwrap()
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn fixture(name: &str) -> Vec<u8> {
    bytes(
        &rows()
            .into_iter()
            .find(|r| r.get("name").unwrap().as_str() == Some(name))
            .unwrap(),
    )
}
fn refused<T>(r: Result<T>) -> Error {
    match r {
        Err(e) => e,
        Ok(_) => panic!("expected original refusal"),
    }
}
fn sentinel() -> Error {
    Error::new(
        ErrorCode::OperationCancelled,
        "retained layout raw sentinel",
    )
    .at(23)
}
fn expected(native: u64, wasm: u64) -> u64 {
    if usize::BITS == 64 { native } else { wasm }
}
#[test]
fn independent_target_controls_and_defining_source_layouts() {
    assert_eq!(size_of::<RetainedSourceLayoutSummary8>(), 168);
    assert_eq!(
        size_of::<PortableRetainedSourceLayout8<'static, 'static>>() as u64,
        expected(216, 200)
    );
    assert_eq!(control_bytes().unwrap(), expected(1040, 936));
    assert_eq!(size_of::<Document>() as u64, expected(144, 104));
    assert_eq!(size_of::<Layer>() as u64, expected(352, 304));
    assert_eq!(size_of::<FilterInstance>() as u64, expected(184, 176));
    assert_eq!(
        size_of::<PortableSource8Plan<'static>>() as u64,
        expected(2656, 1528)
    );
    assert_eq!(census::control_bytes(), expected(9416, 7260));
    let q = ReservedDecodeAdmission::control_quote().unwrap();
    assert_eq!(q.tile_header_bytes, expected(200, 168));
    assert_eq!(
        BudgetLease::control_allocation_quote().unwrap(),
        expected(40, 24)
    );
    assert_eq!(
        coverage_mask::canonical_a8_storage_arc_quote().unwrap(),
        expected(112, 64)
    );
}
#[test]
fn twelve_independent_wires_layouts_counts_and_literal_work() {
    for row in rows() {
        let input = bytes(&row);
        let source = input.clone();
        let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
        let scope = SharedBudget::new(4 << 20).scope(AllocationClass::PaintMetadata);
        let used = Cell::new(0u64);
        let spend = |n| {
            used.set(used.get() + n);
            Ok(())
        };
        let check = || Ok(());
        let plan = checked
            .portable_source8_plan_checked(Default::default(), &scope, &check, &spend)
            .unwrap();
        let before = used.get();
        let live = scope.footprint().live;
        let recipe = plan.retained_source_layout_checked().unwrap();
        let s = recipe.summary();
        let facts = plan.facts();
        assert_eq!(s.layers(), facts.layers);
        assert_eq!(s.tiles(), facts.tiles);
        assert_eq!(s.filters(), facts.filters);
        assert_eq!(s.bands(), facts.bands);
        assert_eq!(s.runs(), facts.runs);
        assert_eq!(s.string_bytes(), facts.string_bytes);
        assert_eq!(s.metadata_bytes(), facts.metadata_bytes);
        assert_eq!(s.scratch_bytes(), facts.maximum_decoder_bytes);
        assert_eq!(
            s.metadata_bytes(),
            row.get("encoded_metadata_bytes").unwrap().as_i64().unwrap() as u64
        );
        assert_eq!(s.doc_inline(), expected(144, 104));
        if let Some(work) = row.get("work").and_then(json::Value::as_i64) {
            assert_eq!(used.get() - before, work as u64, "{:?}", row.get("name"));
        }
        for (key, value) in [
            ("layers", s.layers()),
            ("filters", s.filters()),
            ("tiles", s.tiles()),
            ("mask_present", s.mask_present()),
            ("mask_nonempty", s.mask_nonempty()),
            ("bands", s.bands()),
            ("runs", s.runs()),
            ("string_bytes", s.string_bytes()),
            ("attachment_count", s.attachment_count()),
            ("profile_bytes", s.profile_bytes()),
            ("scratch_bytes", s.scratch_bytes()),
        ] {
            if let Some(v) = row.get(key).and_then(json::Value::as_i64) {
                assert_eq!(value, v as u64, "{key}");
            }
        }
        assert_eq!(scope.footprint().live, live + expected(1040, 936));
        assert_eq!(plan.work_used(), used.get());
        assert_eq!(input, source);
        assert!(format!("{recipe:?}").len() < 160);
        drop(recipe);
        assert_eq!(scope.footprint().live, live);
        drop(plan);
        assert_eq!(scope.footprint().live, 0);
    }
}
#[test]
fn retained_requests_separate_shadow_domains_and_inline_document() {
    for (name, m_struct, tile, cov, icc, a, legacy, masks) in [
        ("baseline", expected(1508, 1316), 0, 0, 0, 0, 0, 0),
        (
            "default-raw",
            expected(1624, 1432),
            expected(240, 192),
            0,
            0,
            0,
            0,
            0,
        ),
        ("empty-own", expected(1508, 1316), 0, 0, 0, 0, 0, 0),
        (
            "nonempty-disabled-own",
            expected(1508, 1316),
            0,
            expected(112, 64),
            0,
            expected(168, 112),
            0,
            1,
        ),
        (
            "icc-profile-none",
            expected(1508, 1316),
            0,
            0,
            expected(320, 280),
            0,
            expected(488, 472),
            0,
        ),
        (
            "mixed-duplicate-masks",
            expected(2060, 1844),
            0,
            expected(336, 192),
            0,
            expected(504, 336),
            0,
            3,
        ),
    ] {
        let input = fixture(name);
        let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
        let scope = SharedBudget::new(4 << 20).scope(AllocationClass::PaintMetadata);
        let check = || Ok(());
        let spend = |_| Ok(());
        let plan = checked
            .portable_source8_plan_checked(Default::default(), &scope, &check, &spend)
            .unwrap();
        let recipe = plan.retained_source_layout_checked().unwrap();
        let s = recipe.summary();
        assert_eq!(s.m_struct(), m_struct, "{name}");
        assert_eq!(s.m_tile(), tile);
        assert_eq!(s.m_cov(), cov);
        assert_eq!(s.m_icc(), icc);
        assert_eq!(s.a_retained(), a);
        assert_eq!(s.icc_retained(), legacy);
        assert_eq!(s.mask_nonempty(), masks);
        assert_eq!(s.decoded_bytes(), s.tiles() * 262144);
        if s.tiles() == 0 {
            assert_eq!(s.q_fixed(), 0);
        } else {
            assert!(s.q_fixed() > 0);
        }
        // Inline Document is a diagnostic component of future Payload, not M.
        assert_eq!(s.doc_inline(), expected(144, 104));
    }
}
#[test]
fn global_limits_saturate_across_two_layers_and_smaller_source_limits_refuse_upstream() {
    let input = fixture("global-two-masks");
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    let scope = SharedBudget::new(4 << 20).scope(AllocationClass::PaintMetadata);
    let check = || Ok(());
    let spend = |_| Ok(());
    let limits = PortableSource8Limits {
        coverage: coverage_mask::Limits {
            bands: 2,
            runs: 2,
            work: 6,
        },
        ..Default::default()
    };
    let plan = checked
        .portable_source8_plan_checked(limits, &scope, &check, &spend)
        .unwrap();
    let recipe = plan.retained_source_layout_checked().unwrap();
    assert_eq!(
        (
            recipe.summary().bands(),
            recipe.summary().runs(),
            recipe.summary().mask_nonempty()
        ),
        (2, 2, 2)
    );
    assert_eq!(recipe.summary().m_cov(), expected(224, 128));
    assert_eq!(recipe.summary().a_retained(), expected(336, 224));
    for coverage in [
        coverage_mask::Limits {
            bands: 1,
            ..limits.coverage
        },
        coverage_mask::Limits {
            work: 5,
            ..limits.coverage
        },
    ] {
        assert_eq!(
            refused(checked.portable_source8_plan_checked(
                PortableSource8Limits { coverage, ..limits },
                &scope,
                &check,
                &spend
            ))
            .code,
            ErrorCode::ResourceLimit
        );
    }
}
#[test]
fn repeated_recipes_keep_same_paid_meter_and_original_borrow() {
    let input = fixture("baseline");
    let original = input.clone();
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    let scope = SharedBudget::new(4 << 20).scope(AllocationClass::PaintMetadata);
    let used = Cell::new(0u64);
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let check = || Ok(());
    let plan = checked
        .portable_source8_plan_checked(Default::default(), &scope, &check, &spend)
        .unwrap();
    let before = used.get();
    let live = scope.footprint().live;
    let first = plan.retained_source_layout_checked().unwrap();
    let second = plan.retained_source_layout_checked().unwrap();
    assert!(std::ptr::eq(first._plan, second._plan));
    assert_eq!(first.summary(), second.summary());
    assert_eq!(used.get() - before, 2 * 13188);
    assert_eq!(plan.work_used(), used.get());
    assert_eq!(scope.footprint().live, live + 2 * expected(1040, 936));
    drop(first);
    assert_eq!(scope.footprint().live, live + expected(1040, 936));
    drop(second);
    assert_eq!(scope.footprint().live, live);
    assert_eq!(input, original);
}
