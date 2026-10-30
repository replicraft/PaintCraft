use super::*;
use crate::session::Session;
use std::sync::Arc;
fn owner() -> Session {
    Session::new(
        paintcraft_document::new_document(
            3,
            2,
            Some(Rgba([19, 31, 47, 0])),
            7,
            paintcraft_document::POLICY,
        )
        .unwrap(),
    )
}
fn batch(s: &Session, operations: &str) -> Batch {
    Batch::from_json(format!(r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{operations}]}}"#,s.document().state_id,s.ticket().publication_epoch).as_bytes()).unwrap()
}
fn add(layer: Uuid, alias: &str) -> String {
    format!(
        r#"{{"op":"filter.add","layer_ref":{{"id":"{layer}"}},"ref":"{alias}","descriptor":{{"kind":"invert-rgb-v1"}}}}"#
    )
}
fn action(layer: Uuid, op: &str, filter: Uuid, tail: &str) -> String {
    format!(
        r#"{{"op":"filter.{op}","layer_ref":{{"id":"{layer}"}},"filter_ref":{{"id":"{filter}"}}{tail}}}"#
    )
}
#[test]
fn atomic_aliases_deterministic_ids_order_and_undo_preserve_raw_sources() {
    let mut a = owner();
    let mut b = owner();
    let layer = a.document().roots[0];
    let before = a.snapshot();
    let source = before.layers[&layer].raster.clone();
    let operations = format!(
        r#"{},{{"op":"filter.duplicate","layer_ref":{{"id":"{layer}"}},"filter_ref":{{"ref":"first"}},"ref":"copy"}},{{"op":"filter.set_enabled","layer_ref":{{"id":"{layer}"}},"filter_ref":{{"ref":"first"}},"enabled":false}},{{"op":"filter.reorder","layer_ref":{{"id":"{layer}"}},"filter_ref":{{"ref":"copy"}},"to_index":0}}"#,
        add(layer, "first")
    );
    let operations = format!(
        r#"{operations},{{"op":"filter.update","layer_ref":{{"id":"{layer}"}},"filter_ref":{{"ref":"first"}},"descriptor":{{"kind":"brightness-contrast-v1","brightness":-10,"contrast":50}}}}"#
    );
    let ba = batch(&a, &operations);
    let bb = batch(&b, &operations);
    let ra = a.apply(&ba).unwrap();
    let rb = b.apply(&bb).unwrap();
    assert_eq!(ra, rb);
    assert!(ra.aliases.is_empty());
    assert_eq!(ra.filter_aliases.len(), 2);
    assert_eq!(
        ra.to_value().get("schema").unwrap().as_str(),
        Some("paintcraft.result/v2")
    );
    assert_eq!(a.undo_depth(), 1);
    assert!(a.is_dirty());
    let filters = &a.document().layers[&layer].filters;
    assert_eq!(filters.as_slice()[0].id(), ra.filter_aliases["copy"].1);
    assert_eq!(filters.as_slice()[1].id(), ra.filter_aliases["first"].1);
    assert_ne!(filters.as_slice()[0].id(), filters.as_slice()[1].id());
    assert!(!filters.as_slice()[1].enabled());
    assert_eq!(
        filters.as_slice()[1].descriptor(),
        paintcraft_core::types::filters::Descriptor::BrightnessContrastV1(
            paintcraft_core::types::filters::BrightnessContrastV1::new(-10, 50).unwrap()
        )
    );
    assert_eq!(
        filters.as_slice()[0].descriptor(),
        paintcraft_core::types::filters::Descriptor::InvertRgbV1
    );

    assert_eq!(a.document().layers[&layer].raster, source);
    let changed = a.snapshot();
    a.undo().unwrap();
    assert!(a.document().same_semantics(&before));
    a.redo().unwrap();
    assert!(a.document().same_semantics(&changed));
}
#[test]
fn exact_noops_keep_epoch_history_identity_and_empty_receipt_shape() {
    let mut s = owner();
    let layer = s.document().roots[0];
    let receipt = s.apply(&batch(&s, &add(layer, "f"))).unwrap();
    let filter = receipt.filter_aliases["f"].1;
    let before = s.snapshot();
    let ticket = s.ticket();
    let history = s.history_view();
    for op in [
        action(
            layer,
            "update",
            filter,
            r#", "descriptor":{"kind":"invert-rgb-v1"}"#,
        ),
        action(layer, "set_enabled", filter, r#", "enabled":true"#),
        action(layer, "reorder", filter, r#", "to_index":0"#),
    ] {
        let receipt = s.apply(&batch(&s, &op)).unwrap();
        assert!(!receipt.committed);
        assert!(receipt.filter_aliases.is_empty());
        assert_eq!(
            receipt.to_value().get("schema").unwrap().as_str(),
            Some("paintcraft.result/v1")
        );
        assert!(receipt.to_value().get("filter_aliases").is_none());
        assert!(Arc::ptr_eq(&before, &s.snapshot()));
        assert_eq!(ticket, s.ticket());
        assert_eq!(history, s.history_view());
    }
}
#[test]
fn references_bad_indices_alias_tombstones_and_late_failure_are_atomic() {
    let mut s = owner();
    let layer = s.document().roots[0];
    let before = s.snapshot();
    let ticket = s.ticket();
    for operations in [
        format!("{},{}", add(layer, "f"), add(layer, "f")),
        format!(
            r#"{},{{"op":"filter.reorder","layer_ref":{{"id":"{layer}"}},"filter_ref":{{"ref":"f"}},"to_index":1}}"#,
            add(layer, "f")
        ),
        format!(
            r#"{},{{"op":"filter.remove","layer_ref":{{"id":"{layer}"}},"filter_ref":{{"ref":"f"}}}},{{"op":"filter.set_enabled","layer_ref":{{"id":"{layer}"}},"filter_ref":{{"ref":"f"}},"enabled":false}}"#,
            add(layer, "f")
        ),
    ] {
        assert!(s.apply(&batch(&s, &operations)).is_err());
        assert!(Arc::ptr_eq(&before, &s.snapshot()));
        assert_eq!(ticket, s.ticket());
        assert_eq!(s.undo_depth(), 0);
    }
    let operations = format!(
        r#"{},{{"op":"filter.remove","layer_ref":{{"id":"{layer}"}},"filter_ref":{{"ref":"f"}}}}"#,
        add(layer, "f")
    );
    let receipt = s.apply(&batch(&s, &operations)).unwrap();
    assert!(!receipt.committed);
    assert!(receipt.filter_aliases.is_empty());
    assert_eq!(
        receipt.to_value().get("schema").unwrap().as_str(),
        Some("paintcraft.result/v1")
    );
    assert_eq!(ticket, s.ticket());
}
fn refuse_filters(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<()> {
    check()?;
    if doc.layers.values().any(|l| !l.filters.is_empty()) {
        Err(Error::new(
            ErrorCode::ResourceLimit,
            "test owner persistence policy",
        ))
    } else {
        Ok(())
    }
}
#[test]
fn persistence_admission_stale_and_cancelled_preparations_publish_no_aliases_or_history() {
    let mut s = owner().with_persistent_validator(Some(refuse_filters));
    let layer = s.document().roots[0];
    let before = s.snapshot();
    let ticket = s.ticket();
    let history = s.history_view();
    let b = batch(&s, &add(layer, "f"));
    assert_eq!(s.apply(&b).unwrap_err().code, ErrorCode::ResourceLimit);
    let prepared = s.preparation().unwrap().apply(&b, || Ok(())).unwrap();
    assert_eq!(
        s.publish_prepared(prepared, || Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(Arc::ptr_eq(&before, &s.snapshot()));
    assert_eq!(ticket, s.ticket());
    assert_eq!(history, s.history_view());
    s.set_persistent_validator(None);
    let b = batch(&s, &add(layer, "f"));
    let prepared = s.preparation().unwrap().apply(&b, || Ok(())).unwrap();
    let cancelled = || {
        Err(Error::new(
            ErrorCode::OperationCancelled,
            "test cancellation",
        ))
    };
    assert_eq!(
        s.publish_prepared(prepared, cancelled).unwrap_err().code,
        ErrorCode::OperationCancelled
    );
    let prepared = s.preparation().unwrap().apply(&b, || Ok(())).unwrap();
    s.apply(&batch(
        &s,
        r#"{"op":"selection.set_rect","rect":{"x":0,"y":0,"width":1,"height":1}}"#,
    ))
    .unwrap();
    let current = s.snapshot();
    let current_ticket = s.ticket();
    assert_eq!(
        s.publish_prepared(prepared, || Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert!(Arc::ptr_eq(&current, &s.snapshot()));
    assert_eq!(current_ticket, s.ticket());
}

#[test]
fn per_layer_count_admission_and_cross_layer_aliases_refuse_without_prefix_publication() {
    let mut s = owner();
    let layer = s.document().roots[0];
    let original = s.snapshot();
    let operations = format!(
        r#"{},{{"op":"layer.duplicate","layer_ref":{{"id":"{layer}"}},"ref":"other","name":"Other"}},{{"op":"filter.set_enabled","layer_ref":{{"ref":"other"}},"filter_ref":{{"ref":"first"}},"enabled":false}}"#,
        add(layer, "first")
    );
    assert_eq!(
        s.apply(&batch(&s, &operations)).unwrap_err().code,
        ErrorCode::TargetInvalid
    );
    assert!(Arc::ptr_eq(&original, &s.snapshot()));
    assert_eq!(s.undo_depth(), 0);
    let mut full = (*original).clone();
    full.layers.get_mut(&layer).unwrap().filters = FilterStack::new(
        (0..FILTERS_MAX)
            .map(|index| {
                FilterInstance::new(
                    Uuid::from_u128(index as u128 + 1),
                    true,
                    paintcraft_core::types::filters::Descriptor::InvertRgbV1,
                )
            })
            .collect(),
    )
    .unwrap();
    let mut s = Session::new(full);
    let before = s.snapshot();
    let ticket = s.ticket();
    assert_eq!(
        s.apply(&batch(&s, &add(layer, "overflow")))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(Arc::ptr_eq(&before, &s.snapshot()));
    assert_eq!(ticket, s.ticket());
    assert_eq!(s.undo_depth(), 0);
}
