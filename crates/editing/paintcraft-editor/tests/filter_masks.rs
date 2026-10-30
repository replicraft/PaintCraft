//! Persistent coverage is authoring state, independent from the live selection.
use paintcraft_commands::Batch;
use paintcraft_core::types::{Error, ErrorCode, Rgba, resources::SharedBudget};
use paintcraft_document::{Document, FilterMask, POLICY, new_document};
use paintcraft_editor::{execute, session::Session};
use paintcraft_raster::{Admission, Selection};
use std::{cell::Cell, sync::Arc};
use uuid::Uuid;

fn owner() -> Session {
    Session::new(new_document(4, 3, Some(Rgba([11, 29, 47, 0])), 73, POLICY).unwrap())
}
fn batch(s: &Session, operations: &str) -> Batch {
    Batch::from_json(format!(r#"{{"schema":"paintcraft.batch/v3","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{operations}]}}"#, s.document().state_id, s.ticket().publication_epoch).as_bytes()).unwrap()
}
fn add(layer: Uuid) -> String {
    format!(
        r#"{{"op":"filter.add","layer_ref":{{"id":"{layer}"}},"ref":"f","descriptor":{{"kind":"invert-rgb-v1"}}}}"#
    )
}
fn action(layer: Uuid, filter: Uuid, kind: &str, tail: &str) -> String {
    format!(
        r#"{{"op":"filter.mask.{kind}","layer_ref":{{"id":"{layer}"}},"filter_ref":{{"id":"{filter}"}}{tail}}}"#
    )
}
fn set(layer: Uuid, filter: Uuid) -> String {
    action(
        layer,
        filter,
        "set",
        r#", "mask":{"domain":{"x":-1,"y":-1,"width":4,"height":3},"bands":[{"y0":0,"y1":1,"runs":[{"x0":0,"x1":1,"coverage":128},{"x0":1,"x1":2,"coverage":255}]}]}"#,
    )
}
fn filter(s: &mut Session) -> (Uuid, Uuid) {
    let layer = s.document().roots[0];
    let receipt = s.apply(&batch(s, &add(layer))).unwrap();
    (layer, receipt.filter_aliases["f"].1)
}
fn stored(doc: &Document, layer: Uuid, filter: Uuid) -> &FilterMask {
    doc.layers[&layer]
        .filters
        .iter()
        .find(|f| f.id() == filter)
        .unwrap()
        .mask()
        .unwrap()
}
#[test]
fn persistent_numeric_mask_noops_updates_duplicates_and_minimal_receipts() {
    let mut s = owner();
    let (layer, id) = filter(&mut s);
    let source = s.snapshot().layers[&layer].raster.clone();
    let first = s.apply(&batch(&s, &set(layer, id))).unwrap();
    assert_eq!(
        first.to_value().get("schema").unwrap().as_str(),
        Some("paintcraft.result/v3")
    );
    assert_eq!(
        first.document_features,
        ["layer-point-filters.v1", "layer-filter-masks.v1"]
    );
    let m = stored(s.document(), layer, id);
    assert_eq!(m.coverage().domain().x0, -1);
    assert_eq!(
        [
            m.coverage().coverage(0, 0),
            m.coverage().coverage(1, 0),
            m.coverage().coverage(2, 0)
        ],
        [128, 255, 0]
    );
    let before = s.snapshot();
    let depth = s.undo_depth();
    assert!(!s.apply(&batch(&s, &set(layer, id))).unwrap().committed);
    assert_eq!(depth, s.undo_depth());
    assert!(Arc::ptr_eq(&before, &s.snapshot()));
    let ops = format!(
        r#"{{"op":"filter.update","layer_ref":{{"id":"{layer}"}},"filter_ref":{{"id":"{id}"}},"descriptor":{{"kind":"brightness-contrast-v1","brightness":20,"contrast":0}}}},{{"op":"filter.set_enabled","layer_ref":{{"id":"{layer}"}},"filter_ref":{{"id":"{id}"}},"enabled":false}},{{"op":"filter.duplicate","layer_ref":{{"id":"{layer}"}},"filter_ref":{{"id":"{id}"}},"ref":"copy"}}"#
    );
    let receipt = s.apply(&batch(&s, &ops)).unwrap();
    let copied = receipt.filter_aliases["copy"].1;
    assert_eq!(
        stored(s.document(), layer, id),
        stored(s.document(), layer, copied)
    );
    assert!(!s.document().layers[&layer].filters.as_slice()[0].enabled());
    assert_eq!(s.document().layers[&layer].raster, source);
    s.undo().unwrap();
    assert!(s.document().same_semantics(&before));
    s.redo().unwrap();
    let remove = format!(
        "{},{}",
        action(layer, id, "remove", ""),
        action(layer, copied, "remove", "")
    );
    let receipt = s.apply(&batch(&s, &remove)).unwrap();
    assert_eq!(
        receipt.to_value().get("schema").unwrap().as_str(),
        Some("paintcraft.result/v1")
    );
    assert!(receipt.document_features.is_empty());
    assert!(
        !s.apply(&batch(&s, &action(layer, id, "remove", "")))
            .unwrap()
            .committed
    );
}
#[test]
fn selection_is_frozen_at_operation_and_zero_disabled_masks_remain_stored() {
    let mut s = owner();
    let (layer, id) = filter(&mut s);
    let ops = format!(
        r#"{{"op":"selection.set_rect","rect":{{"x":1,"y":0,"width":2,"height":1}}}}, {}, {{"op":"selection.clear"}}"#,
        action(layer, id, "from_selection", ",\"enabled\":false")
    );
    let before = s.snapshot();
    s.apply(&batch(&s, &ops)).unwrap();
    let mask = stored(s.document(), layer, id);
    assert!(!mask.enabled());
    assert_eq!(
        (0..4)
            .map(|x| mask.coverage().coverage(x, 0))
            .collect::<Vec<_>>(),
        [0, 255, 255, 0]
    );
    let captured = s.snapshot();
    s.undo().unwrap();
    assert!(s.document().same_semantics(&before));
    s.redo().unwrap();
    assert!(s.document().same_semantics(&captured));
    let empty = format!(
        r#"{{"op":"selection.set_rect","rect":{{"x":8,"y":8,"width":1,"height":1}}}},{}"#,
        action(layer, id, "from_selection", "")
    );
    let receipt = s.apply(&batch(&s, &empty)).unwrap();
    assert!(!receipt.document_features.is_empty());
    assert_eq!(stored(s.document(), layer, id).coverage().coverage(0, 0), 0);
    assert_eq!(stored(s.document(), layer, id).coverage().stats().runs, 0);
    s.apply(&batch(&s, r#"{"op":"selection.clear"}"#)).unwrap();
    let before = s.snapshot();
    assert_eq!(
        s.apply(&batch(&s, &action(layer, id, "from_selection", "")))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert!(Arc::ptr_eq(&before, &s.snapshot()));
}
#[test]
fn arbitrary_inverted_selection_captures_exact_source_local_coverage() {
    let mut s = owner();
    let (layer, id) = filter(&mut s);
    let ops = format!(
        r#"{{"op":"selection.set_polygon","points":[{{"x":0,"y":0}},{{"x":3,"y":0}},{{"x":0,"y":3}}]}},{{"op":"selection.invert"}},{}"#,
        action(layer, id, "from_selection", "")
    );
    s.apply(&batch(&s, &ops)).unwrap();
    let mask = stored(s.document(), layer, id);
    assert_eq!(mask.coverage().coverage(0, 0), 0);
    assert_eq!(mask.coverage().coverage(3, 2), 255);
    assert_eq!(mask.coverage().coverage(4, 2), 0);
}
#[test]
fn admitted_captures_release_on_failure_cancellation_and_stale_preparation() {
    let mut s = owner();
    let (layer, id) = filter(&mut s);
    let op = batch(&s, &set(layer, id));
    let before = s.snapshot();
    let budget = SharedBudget::new(1 << 20);
    let staged = execute::execute_with_admission(
        &before,
        Selection::Inactive,
        &op,
        Admission::new(u64::MAX).with_coverage_budget(budget.clone()),
    )
    .unwrap();
    assert!(budget.usage().reserved > 0);
    let usage = budget.usage().reserved;
    let cloned = staged.doc.clone();
    assert_eq!(budget.usage().reserved, usage);
    drop(staged);
    assert!(budget.usage().reserved > 0);
    drop(cloned);
    assert_eq!(budget.usage().reserved, 0);
    let tiny = SharedBudget::new(1);
    assert_eq!(
        execute::execute_with_admission(
            &before,
            Selection::Inactive,
            &op,
            Admission::new(u64::MAX).with_coverage_budget(tiny.clone())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(tiny.usage().reserved, 0);
    let calls = Cell::new(0);
    let result = execute::execute_with_admission_checked(
        &before,
        Selection::Inactive,
        &op,
        Admission::new(u64::MAX).with_coverage_budget(budget.clone()),
        &|| {
            calls.set(calls.get() + 1);
            if budget.usage().reserved > 0 {
                Err(Error::new(ErrorCode::OperationCancelled, "after admission"))
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    assert!(calls.get() > 0);
    assert_eq!(budget.usage().reserved, 0);
    let prepared = s.preparation().unwrap().apply(&op, || Ok(())).unwrap();
    s.apply(&batch(
        &s,
        r#"{"op":"selection.set_rect","rect":{"x":0,"y":0,"width":1,"height":1}}"#,
    ))
    .unwrap();
    let unchanged = s.snapshot();
    assert_eq!(
        s.publish_prepared(prepared, || Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert!(Arc::ptr_eq(&unchanged, &s.snapshot()));
    assert!(stored_mask_absent(s.document(), layer, id));
}
fn stored_mask_absent(doc: &Document, layer: Uuid, filter: Uuid) -> bool {
    doc.layers[&layer]
        .filters
        .iter()
        .find(|entry| entry.id() == filter)
        .unwrap()
        .mask()
        .is_none()
}
#[test]
fn crop_and_whole_source_orientation_map_masks_once_with_pixels() {
    let mut s = owner();
    let (layer, id) = filter(&mut s);
    s.apply(&batch(&s, &set(layer, id))).unwrap();
    s.apply(&batch(
        &s,
        r#"{"op":"canvas.transform","transform":"rotate-right"}"#,
    ))
    .unwrap();
    assert_eq!(s.document().canvas, (3, 4));
    let mask = stored(s.document(), layer, id);
    assert_eq!(mask.coverage().coverage(2, 0), 128);
    assert_eq!(mask.coverage().coverage(2, 1), 255);
    s.apply(&batch(
        &s,
        r#"{"op":"canvas.crop","rect":{"x":2,"y":0,"width":1,"height":2}}"#,
    ))
    .unwrap();
    let mask = stored(s.document(), layer, id);
    assert_eq!(
        [
            mask.coverage().coverage(0, 0),
            mask.coverage().coverage(0, 1)
        ],
        [128, 255]
    );
    let cropped = s.snapshot();
    s.undo().unwrap();
    s.redo().unwrap();
    assert!(s.document().same_semantics(&cropped));
}
#[test]
fn selected_pixel_rebase_keeps_spatial_mask_fixed_and_reuses_capture() {
    let mut s = owner();
    let (layer, id) = filter(&mut s);
    s.apply(&batch(&s, &set(layer, id))).unwrap();
    let ops = format!(
        r##"{{"op":"selection.set_rect","rect":{{"x":0,"y":0,"width":1,"height":1}}}},{{"op":"raster.move_selection","layer_ref":{{"id":"{layer}"}},"offset":{{"x":-2,"y":0}},"clear":"#00000000"}}"##
    );
    s.apply(&batch(&s, &ops)).unwrap();
    assert_eq!(s.document().layers[&layer].origin, (-2, 0));
    let mask = stored(s.document(), layer, id);
    assert_eq!(
        [
            mask.coverage().coverage(0, 0),
            mask.coverage().coverage(2, 0),
            mask.coverage().coverage(3, 0)
        ],
        [0, 128, 255]
    );
}

#[test]
fn scalar_mask_noops_need_no_new_coverage_capacity_and_keep_owner_history() {
    let mut s = owner();
    let (layer, id) = filter(&mut s);
    for (kind, tail) in [("remove", ""), ("set_enabled", ",\"enabled\":true")] {
        if kind == "set_enabled" {
            s.apply(&batch(&s, &set(layer, id))).unwrap();
        }
        let before = s.snapshot();
        let ticket = s.ticket();
        let depth = s.undo_depth();
        let budget = SharedBudget::new(0);
        let prepared = s
            .preparation()
            .unwrap()
            .apply_with_admission(
                &batch(&s, &action(layer, id, kind, tail)),
                Admission::new(u64::MAX).with_coverage_budget(budget.clone()),
                || Ok(()),
            )
            .unwrap();
        let receipt = s.publish_prepared(prepared, || Ok(())).unwrap();
        assert!(!receipt.committed);
        assert!(Arc::ptr_eq(&before, &s.snapshot()));
        assert_eq!(s.ticket(), ticket);
        assert_eq!(s.undo_depth(), depth);
        assert_eq!(budget.usage().reserved, 0);
        assert_eq!(budget.usage().peak, 0);
    }
}
#[test]
fn selected_rebase_preserves_negative_domain_and_latent_off_layer_coverage() {
    let mut s = owner();
    let (layer, id) = filter(&mut s);
    let numeric = action(
        layer,
        id,
        "set",
        r#", "mask":{"domain":{"x":-3,"y":-2,"width":9,"height":6},"bands":[{"y0":-1,"y1":0,"runs":[{"x0":-3,"x1":-2,"coverage":57}]},{"y0":0,"y1":1,"runs":[{"x0":0,"x1":1,"coverage":128}]}]}"#,
    );
    s.apply(&batch(&s, &numeric)).unwrap();
    let original = stored(s.document(), layer, id).clone();
    for (dx, origin) in [(1, (0, 0)), (-2, (-2, 0))] {
        let ops = format!(
            r##"{{"op":"selection.set_rect","rect":{{"x":0,"y":0,"width":1,"height":1}}}},{{"op":"raster.move_selection","layer_ref":{{"id":"{layer}"}},"offset":{{"x":{dx},"y":0}},"clear":"#00000000"}}"##
        );
        s.apply(&batch(&s, &ops)).unwrap();
        assert_eq!(s.document().layers[&layer].origin, origin);
        let current = stored(s.document(), layer, id);
        assert_eq!(
            current.coverage().domain(),
            original
                .coverage()
                .domain()
                .translated(-origin.0, -origin.1)
                .unwrap()
        );
        for y in -3..6 {
            for x in -5..10 {
                assert_eq!(
                    current.coverage().coverage(x - origin.0, y - origin.1),
                    original.coverage().coverage(x, y),
                    "document sample {x},{y}"
                );
            }
        }
    }
}
