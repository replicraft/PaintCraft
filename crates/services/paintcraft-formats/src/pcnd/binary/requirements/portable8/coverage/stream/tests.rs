use super::*;
use paintcraft_core::types::{
    json::{self, Value},
    resources::{AllocationClass, SharedBudget},
};
use std::cell::{Cell, RefCell};
mod controls;
const WORK: u64 = 268435456;
fn native() -> bool {
    cfg!(target_pointer_width = "64")
}
fn target() -> &'static str {
    if native() { "native64" } else { "wasm32" }
}
fn owner_c() -> u64 {
    if native() { 224 } else { 164 }
}
fn stream_c() -> u64 {
    if native() { 1552 } else { 1380 }
}
fn rows() -> Vec<Value> {
    json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/source8-intention-stream-fixtures/oracle.json"
    )))
    .unwrap()
    .get("cases")
    .unwrap()
    .as_array()
    .unwrap()
    .to_vec()
}
fn row(name: &str) -> Value {
    rows()
        .into_iter()
        .find(|r| r.get("name").unwrap().as_str() == Some(name))
        .unwrap()
}
fn n(v: &Value, k: &str) -> u64 {
    v.get(k).unwrap().as_i64().unwrap() as u64
}
fn params(v: &Value) -> &Value {
    v.get("targets").unwrap().get(target()).unwrap()
}
fn bytes(v: &Value) -> Vec<u8> {
    v.get("hex")
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
fn limits(v: &Value, work: Option<u64>) -> PortableSource8Limits {
    let mut l = PortableSource8Limits::default();
    l.coverage.bands = n(v, "bands") as usize;
    l.coverage.runs = n(v, "runs") as usize;
    l.coverage.work = work.unwrap_or(67108864);
    l
}
type Owned = (IntentionReceipt8, Vec<FilterInstance>, Vec<LayerMask>);
fn read(owner: &PortableCoverageMaterializer8<'_, '_>, layers: usize) -> Result<Owned> {
    let mut filters = Vec::new();
    let mut masks = Vec::new();
    let mut document = owner.document_intentions_checked()?;
    for index in 0..layers {
        let (header, mut layer) = document.begin_layer()?;
        assert_eq!(header.name, "Layer");
        assert_eq!(
            header.blend_mode,
            if index == 0 {
                paintcraft_core::types::LayerBlendMode::Multiply
            } else {
                paintcraft_core::types::LayerBlendMode::Normal
            }
        );
        for _ in 0..header.filters {
            let (next, value) = layer.next_filter()?;
            layer = next;
            filters.push(value.ok_or_else(invalid)?);
        }
        let (next, mask) = layer.finish_layer()?;
        document = next;
        if let Some(mask) = mask {
            masks.push(mask);
        }
    }
    Ok((document.finish()?, filters, masks))
}
#[test]
fn exact_defining_layouts_and_bounded_debug_are_callback_free() {
    assert_eq!(
        size_of::<DocumentIntentions8<'static, 'static, 'static>>(),
        if native() { 104 } else { 80 }
    );
    assert_eq!(
        size_of::<LayerIntentions8<'static, 'static, 'static>>(),
        if native() { 200 } else { 160 }
    );
    assert_eq!(
        size_of::<FilterInstance>(),
        if native() { 184 } else { 176 }
    );
    assert_eq!(size_of::<FilterMask>(), if native() { 152 } else { 144 });
    assert_eq!(size_of::<LayerMask>(), if native() { 152 } else { 144 });
    assert_eq!(
        size_of::<PortableFilterView<'static>>(),
        if native() { 104 } else { 96 }
    );
    assert_eq!(stream_control_bytes().unwrap(), stream_c());
    let r = row("one-unmasked");
    let b = bytes(&r);
    let checked = CheckedContainer::verify(&b, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(16 << 20);
    let ps = budget.scope(AllocationClass::PaintMetadata);
    let sc = budget
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::CoverageMasks);
    let m = budget.account(AllocationClass::PaintMetadata);
    let used = Cell::new(0u64);
    let calls = Cell::new(0u64);
    let spend = |n| {
        used.set(used.get() + n);
        Ok(())
    };
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let plan = checked
        .portable_source8_plan_checked(limits(&r, None), &ps, &check, &spend)
        .unwrap();
    let owner = plan.coverage_materializer(&sc, &m).unwrap();
    let doc = owner.document_intentions_checked().unwrap();
    let before = (used.get(), calls.get());
    assert!(format!("{doc:?}").len() < 180);
    assert_eq!((used.get(), calls.get()), before);
    let (_, layer) = doc.begin_layer().unwrap();
    let before = (used.get(), calls.get());
    assert!(format!("{layer:?}").len() < 180);
    assert_eq!((used.get(), calls.get()), before);
    drop(layer);
    assert_eq!(sc.footprint().live, owner_c());
}
#[test]
fn independent_whole_document_wires_have_exact_work_flags_counts_and_distinct_owners() {
    for r in rows() {
        let b = bytes(&r);
        let original = b.clone();
        let checked = CheckedContainer::verify(&b, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(32 << 20);
        let ps = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let sc = budget
            .reserve_capacity(n(params(&r), "A_scope_exact"))
            .unwrap()
            .scope(AllocationClass::CoverageMasks);
        let mut m = budget.account(AllocationClass::PaintMetadata);
        m.resize(n(params(&r), "M")).unwrap();
        let used = Cell::new(0u64);
        let calls = Cell::new(0u64);
        let trace = RefCell::new(Vec::new());
        let armed = Cell::new(false);
        let spend = |v| {
            used.set(
                used.get()
                    .checked_add(v)
                    .filter(|n| *n <= WORK)
                    .ok_or_else(bound)?,
            );
            if armed.get() {
                trace.borrow_mut().push(v);
            }
            Ok(())
        };
        let check = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        let l = limits(
            &r,
            if r.get("name").unwrap().as_str() == Some("two-layer-global") {
                Some(65536)
            } else {
                None
            },
        );
        let result = checked.portable_source8_plan_checked(l, &ps, &check, &spend);
        if r.get("accept").unwrap().as_bool() == Some(false) {
            assert_eq!(result.unwrap_err().code, ErrorCode::CodecFailed);
            assert_eq!(ps.footprint().live, 0);
            assert_eq!(sc.footprint().live, 0);
            assert_eq!(b, original);
            continue;
        }
        let plan = result.unwrap();
        let owner = plan.coverage_materializer(&sc, &m).unwrap();
        let before = used.get();
        let before_checks = calls.get();
        armed.set(true);
        let (receipt, filters, masks) = read(&owner, n(&r, "layers") as usize).unwrap();
        assert_eq!(used.get() - before, n(params(&r), "total_work"));
        assert_eq!(calls.get() - before_checks, n(params(&r), "checks"));
        assert_eq!(
            (
                receipt.layers(),
                receipt.filters(),
                receipt.mask_present(),
                receipt.mask_nonempty(),
                receipt.bands(),
                receipt.runs()
            ),
            (
                n(&r, "layers"),
                n(&r, "filters"),
                n(&r, "present"),
                n(&r, "nonempty"),
                n(&r, "bands"),
                n(&r, "runs")
            )
        );
        if let Some(events) = params(&r).get("events") {
            let expected = events
                .as_array()
                .unwrap()
                .iter()
                .map(|e| n(e, "debit"))
                .collect::<Vec<_>>();
            assert_eq!(*trace.borrow(), expected);
        }
        let mut fi = filters.iter();
        let mut own = masks.iter();
        let mut pointers = Vec::new();
        for layer in r.get("source").unwrap().as_array().unwrap() {
            let parts = layer.as_array().unwrap();
            for f in parts[0].as_array().unwrap() {
                let f = f.as_array().unwrap();
                let actual = fi.next().unwrap();
                assert_eq!(actual.enabled(), f[0].as_bool().unwrap());
                assert_eq!(
                    actual.descriptor(),
                    paintcraft_core::types::filters::Descriptor::InvertRgbV1
                );
                let kind = f[1].as_str().unwrap();
                assert_mask(
                    actual.mask().map(|m| (m.enabled(), m.coverage())),
                    kind,
                    &mut pointers,
                );
            }
            let kind = parts[1].as_str().unwrap();
            if kind != "absent" {
                let actual = own.next().unwrap();
                assert_mask(
                    Some((actual.enabled(), actual.coverage())),
                    kind,
                    &mut pointers,
                );
            }
        }
        pointers.sort_unstable();
        pointers.dedup();
        assert_eq!(pointers.len() as u64, n(&r, "nonempty"));
        assert_eq!(sc.footprint().live, owner_c() + n(params(&r), "retained_A"));
        assert_eq!(plan.work_used(), used.get());
        assert_eq!(plan.check_calls(), calls.get());
        drop(filters);
        drop(masks);
        drop(owner);
        assert_eq!(sc.footprint().live, 0);
        drop(plan);
        assert_eq!(ps.footprint().live, 0);
        assert_eq!(b, original);
    }
}
fn assert_mask(actual: Option<(bool, &A8Mask)>, kind: &str, pointers: &mut Vec<usize>) {
    if kind == "absent" {
        assert!(actual.is_none());
        return;
    }
    let (enabled, mask) = actual.unwrap();
    assert_eq!(enabled, kind != "disabled");
    assert_eq!(mask.coverage(0, 0), if kind == "empty" { 0 } else { 128 });
    assert_eq!(
        mask.domain(),
        paintcraft_core::types::Rect::from_origin_size(0, 0, 2, 1).unwrap()
    );
    assert_eq!(
        paintcraft_core::types::policy::hex(&mask.digest()),
        if kind == "empty" {
            "271cf4b812c97b36a868f03fa03a3ea8616ecb806c968b6788d80166c53c13cd"
        } else {
            "04320d091d61cb072917a0d8efacd5d6ef5db6140feecf9e1bc63c2765db0da3"
        }
    );
    if kind == "empty" {
        assert!(mask.storage_usage().is_none());
        assert_eq!((mask.stats().bands, mask.stats().runs), (0, 0));
    } else {
        pointers.push(mask.storage_usage().unwrap().0);
        assert_eq!(mask.stats().bands, 1);
        assert_eq!(mask.stats().runs, 1);
    }
}

#[test]
fn original_normal_only_wires_remain_typed_upstream_refusals() {
    let oracle = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/source8-intention-stream-fixtures/oracle.json"
    )))
    .unwrap();
    let negatives = oracle
        .get("upstream_negative_cases")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(negatives.len(), 11);
    for r in negatives {
        let b = bytes(r);
        let original = b.clone();
        let checked = CheckedContainer::verify(&b, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(32 << 20);
        let ps = budget
            .reserve_capacity(4 << 20)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        let sc = budget
            .reserve_capacity(1 << 20)
            .unwrap()
            .scope(AllocationClass::CoverageMasks);
        let used = Cell::new(0u64);
        let spend = |n| {
            used.set(
                used.get()
                    .checked_add(n)
                    .filter(|n| *n <= WORK)
                    .ok_or_else(bound)?,
            );
            Ok(())
        };
        let check = || Ok(());
        let result = checked.portable_source8_plan_checked(limits(r, None), &ps, &check, &spend);
        assert_eq!(result.unwrap_err().code, ErrorCode::CodecFailed);
        assert_eq!(ps.footprint().live, 0);
        assert_eq!(sc.footprint().live, 0);
        assert_eq!(b, original);
    }
}
