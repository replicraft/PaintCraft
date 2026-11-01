//! Native host/control tests; no actual Worker or browser execution claim.
use super::*;
use paintcraft_core::types::{
    ErrorCode, Rect,
    json::Value,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{Document, LayerMask};
use paintcraft_editor::session::Ticket;
use paintcraft_formats::{export::layer_appearance::Admission7, pcnd::CheckedContainer};
use paintcraft_raster::{
    Admission,
    coverage_mask::{A8Mask, Limits},
};
use std::sync::Arc;
use uuid::Uuid;
fn source() -> Arc<Document> {
    let mut document =
        paintcraft_document::import_image(2, 1, &[9, 17, 33, 0, 64, 16, 128, 255], "source7")
            .unwrap();
    let coverage = A8Mask::uniform(
        Rect::new(0, 0, 2, 1).unwrap(),
        127,
        &SharedBudget::new(4096),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    document.layers.get_mut(&document.roots[0]).unwrap().mask =
        Some(LayerMask::new(false, coverage));
    Arc::new(document)
}
fn captured(document: Arc<Document>) -> (capture::Capture7, preparation::Preparation7) {
    let prefix = preparation::Preparation7::before_capture();
    let capture = capture::Capture7::capture(
        Uuid::from_u128(1),
        Ticket {
            state_id: document.state_id,
            publication_epoch: 9,
        },
        document,
        Uuid::from_u128(2),
        None,
        &prefix,
        &|| Ok(()),
    )
    .unwrap();
    (capture, prefix)
}
#[test]
fn actual_snapshot_seals_prefix_and_binds_source_facts_without_changing_public_document() {
    let document = source();
    let before = document.clone();
    let (capture, prefix) = captured(document.clone());
    prefix.spend(17).unwrap();
    let budget = SharedBudget::new(64 << 20);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let admission = Admission::new(0).with_paint_metadata_scope(scope.clone());
    let prepared = prepare::prepare(capture, prefix, admission, &scope, &|| Ok(()), None).unwrap();
    let bytes = prepared.snapshot.as_ref().unwrap().bytes();
    let (witness, sha) = CheckedContainer::verify_with_source_hash(bytes, &|| Ok(())).unwrap();
    assert_eq!(witness.version(), 7);
    assert_eq!(sha, prepared.source_sha256);
    assert!(prepared.frames.sealed.main_spent >= 17);
    assert_eq!(
        prepared.frames.sealed.main_spent + prepared.frames.sealed.admission.base_work,
        Admission7::BASE_MAX
    );
    assert_eq!(prepared.frames.sealed.admission.noise_work, 0);
    let wrapper = control::final_wrapper(&prepared).unwrap();
    let plan = wrapper.get("plan").unwrap();
    assert_eq!(
        plan.get("document").and_then(Value::as_str),
        Some(document.document_id.to_string().as_str())
    );
    assert_eq!(plan.get("maskCount"), Some(&Value::Int(0))); // old filter-only facts
    assert!(
        plan.get("sourceRequirements")
            .unwrap()
            .get("layerMasks")
            .is_some()
    );
    let view = crate::app::filter_editing::render::View::new(&document, document.canvas_rect(), 0)
        .unwrap();
    let first = control::begin(&prepared, 1, view, true).unwrap();
    let next = control::begin(&prepared, 2, view, true).unwrap();
    assert_eq!(
        first.get("candidateRequirements"),
        next.get("candidateRequirements")
    );
    assert_eq!(first.get("admissionSha256"), next.get("admissionSha256"));
    assert!(Arc::ptr_eq(&document, &before));
    assert!(document.same_semantics(&before));
    assert!(budget.usage().reserved > 0);
    drop(prepared);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn preparation_denial_and_cancellation_leave_source_and_caller_realm_unchanged() {
    let source = source();
    let (capture, prefix) = captured(source.clone());
    let budget = SharedBudget::new(1);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let result = prepare::prepare(capture, prefix, Admission::new(0), &scope, &|| Ok(()), None);
    assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(budget.usage().reserved, 0);
    let (capture, prefix) = captured(source.clone());
    let error = paintcraft_core::types::Error::new(ErrorCode::OperationCancelled, "cancel");
    let result = prepare::prepare(
        capture,
        prefix,
        Admission::new(0),
        &scope,
        &|| Err(error.clone()),
        None,
    );
    assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    assert_eq!(budget.usage().reserved, 0);
    assert!(source.layers[&source.roots[0]].mask.is_some());
}
