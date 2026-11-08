use super::*;
use paintcraft_core::types::{
    json,
    resources::{AllocationClass, SharedBudget},
};
mod controls;
fn rows() -> Vec<json::Value> {
    json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/portable-source8-plan-fixtures/oracle.json"
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
fn error<T>(r: Result<T>) -> Error {
    match r {
        Ok(_) => panic!("expected refusal"),
        Err(e) => e,
    }
}
fn stop() -> Error {
    Error::new(ErrorCode::OperationCancelled, "original portable sentinel").at(19)
}
#[test]
fn thirty_independent_structural_wires_and_view_replay_use_one_authentication() {
    for row in rows() {
        let input = bytes(&row);
        let original = input.clone();
        super::super::super::PAYLOAD_HASH_PASSES.with(|n| n.set(0));
        let verified = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(8 << 20);
        let scope = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let checks = Cell::new(0u64);
        let used = Cell::new(0u64);
        let check = || {
            checks.set(checks.get() + 1);
            Ok(())
        };
        let spend = |n| {
            used.set(used.get().checked_add(n).unwrap());
            Ok(())
        };
        let result =
            verified.portable_source8_plan_checked(Default::default(), &scope, &check, &spend);
        if row.get("accept").unwrap().as_bool() == Some(true) {
            let plan = result.unwrap();
            let facts = plan.facts();
            assert_eq!(facts.layers, 2);
            if let Some(expected) = row.get("tiles").and_then(json::Value::as_i64) {
                assert_eq!(facts.tiles, expected as u64);
            }
            if let Some(expected) = row.get("max_scratch").and_then(json::Value::as_i64) {
                assert_eq!(facts.maximum_decoder_bytes, expected as u64);
            }
            let h = plan.header().unwrap();
            assert_eq!(h.layers, 2);
            assert_eq!(h.canvas, (2, 1));
            assert_eq!(plan.document_id().unwrap(), h.document_id);
            let mut filters = 0;
            let mut tiles = 0;
            for i in 0..facts.layers as usize {
                let layer = plan.layer(i).unwrap();
                let header = layer.header().unwrap();
                for f in 0..header.filters {
                    let view = layer.filter(f).unwrap();
                    let _ = view.descriptor;
                    filters += 1;
                }
                if let Some(mask) = layer.mask().unwrap() {
                    assert!(!mask.wire_bytes().is_empty());
                }
                for j in 0..layer.block_count() {
                    let block = layer.block(j).unwrap();
                    assert!(matches!(block.encoding, 1 | 2 | 4 | 5));
                    assert!(!block.stored_bytes().is_empty());
                    tiles += 1;
                }
            }
            assert_eq!(filters, facts.filters);
            assert_eq!(tiles, facts.tiles);
            assert_eq!(plan.work_used(), used.get());
            assert_eq!(plan.check_calls(), checks.get());
            assert_eq!(error(plan.layer(usize::MAX)).code, ErrorCode::CodecFailed);
            drop(plan);
        } else {
            let e = error(result);
            let expected = row.get("error").unwrap().as_str().unwrap();
            assert_eq!(
                e.code,
                if expected == "CapabilityUnsupported" {
                    ErrorCode::CapabilityUnsupported
                } else {
                    ErrorCode::CodecFailed
                }
            );
        }
        super::super::super::PAYLOAD_HASH_PASSES.with(|n| assert_eq!(n.get(), 1));
        assert_eq!(input, original);
        assert_eq!(scope.footprint().live, 0);
        drop(scope);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn exact_record_shapes_and_no_pixel_or_domain_owner_in_cursors() {
    assert_eq!(size_of::<LayerRecord>(), 56);
    assert_eq!(size_of::<BlockRecord>(), 24);
    let q = portable_source8_control_bytes();
    assert_eq!(q.layer_record, 56);
    assert_eq!(q.block_record, 24);
    let input = fixture("manual-raw");
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    let scope = SharedBudget::new(4 << 20).scope(AllocationClass::PaintMetadata);
    let check = || Ok(());
    let spend = |_| Ok(());
    let p = checked
        .portable_source8_plan_checked(Default::default(), &scope, &check, &spend)
        .unwrap();
    let b = p.block(0, 0).unwrap();
    assert_eq!(b.decoded_bytes, 8);
    let body = checked.body();
    let range = b.stored_bytes();
    assert!(range.as_ptr() >= body.as_ptr());
    assert!((range.as_ptr() as usize + range.len()) <= body.as_ptr() as usize + body.len());
    assert_eq!(p.facts().tiles * 24 + p.facts().layers * 56, 136);
    assert_eq!(
        scope.footprint().live,
        size_of::<PortableSource8Plan<'static>>() as u64 + 136
    );
    drop(p);
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn source_checksum_failure_stays_before_any_plan_or_transport_refusal() {
    let mut input = fixture("manual-raw");
    *input.last_mut().unwrap() ^= 1;
    assert_eq!(
        error(CheckedContainer::verify(&input, &|| Ok(()))).code,
        ErrorCode::CodecFailed
    );
}
#[test]
fn explicit_limits_reject_before_a_plan_can_escape() {
    let input = fixture("manual-runs-lz4");
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    for mode in 0..4 {
        let scope = SharedBudget::new(4 << 20).scope(AllocationClass::PaintMetadata);
        let mut limits = PortableSource8Limits::default();
        match mode {
            0 => limits.layers = 1,
            1 => limits.tiles = 0,
            2 => limits.metadata_bytes = 1,
            _ => limits.canvas_pixels = 1,
        };
        assert_eq!(
            error(checked.portable_source8_plan_checked(limits, &scope, &|| Ok(()), &|_| Ok(())))
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(scope.footprint().live, 0);
    }
}

#[test]
fn bounded_debug_never_calls_helpers_or_dumps_source_payloads() {
    let input = fixture("manual-raw");
    let checked = CheckedContainer::verify(&input, &|| Ok(())).unwrap();
    let scope = SharedBudget::new(4 << 20).scope(AllocationClass::PaintMetadata);
    let used = Cell::new(0u64);
    let checks = Cell::new(0u64);
    let check = || {
        checks.set(checks.get() + 1);
        Ok(())
    };
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let p = checked
        .portable_source8_plan_checked(Default::default(), &scope, &check, &spend)
        .unwrap();
    let layer = p.layer(0).unwrap();
    let header = layer.header().unwrap();
    let block = p.block(0, 0).unwrap();
    let paid = used.get();
    let calls = checks.get();
    let debug = format!("{p:?} {layer:?} {header:?} {block:?}");
    assert!(debug.len() < 2048);
    assert_eq!(used.get(), paid);
    assert_eq!(checks.get(), calls);
    let private = [37u8; 1024];
    let title = "sensitive title must stay private";
    let h = PortableHeaderView {
        document_id: header.id,
        state_id: header.id,
        canvas: (2, 1),
        title: Some(title),
        source_metadata: Some(&private),
        layers: 2,
    };
    let m = PortableMaskView {
        domain: Rect::new(0, 0, 2, 1).unwrap(),
        enabled: true,
        bands: 1,
        runs: 1,
        bytes: &private,
    };
    let text = format!("{h:?} {m:?}");
    assert!(text.len() < 2048);
    assert!(!text.contains(title));
    assert!(!text.contains("37, 37"));
    assert_eq!(used.get(), paid);
    assert_eq!(checks.get(), calls);
}
