//! Injection of a finite transaction work remainder, not a kernel cost oracle.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_document::POLICY;
fn ok() -> Result<()> {
    Ok(())
}
fn stage<'a>(doc: &Document, batch: &'a Batch, budget: &SharedBudget, work: u64) -> Stage<'a> {
    let adm = Admission::new(1 << 20).with_coverage_budget(budget.clone());
    Stage {
        doc: doc.clone(),
        selection: Selection::Inactive,
        selections: planning::storage::Store::new(&adm),
        selection_geometries: planning::storage::Store::new(&adm),
        filter_masks: planning::storage::Store::new(&adm),
        coverage_work: work,
        mask_replay: 0,
        aliases: HashMap::new(),
        filter_aliases: HashMap::new(),
        style_aliases: HashMap::new(),
        style_edit: false,
        _style_charge: None,
        planning: Some(planning::Compiler::new(&adm, batch, &ok).unwrap()),
        operation: 0,
        adm,
        pencil_pixels: 0,
        pencil_segments: 0,
        stroke_rows: 0,
        selection_work: SELECTION_FILTER_WORK_MAX,
        transform_reads: 0,
        base_state: doc.state_id.to_string(),
        digest: &batch.digest,
        check: &ok,
        geometry: None,
    }
}
#[test]
fn successive_layer_actions_share_transaction_work_and_failure_releases_all_new_fields() {
    let mut d =
        paintcraft_document::new_document(4, 3, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    let id = d.roots[0];
    let source = SharedBudget::new(8192);
    d.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(
        false,
        A8Mask::from_bands(
            Rect::new(0, 0, 4, 3).unwrap(),
            &[],
            &source,
            Limits::default(),
            &ok,
        )
        .unwrap(),
    ));
    let op = LayerMaskOp::Invert {
        layer: LayerRef::Id(id),
    };
    let batch = Batch {
        version: 7,
        policy: POLICY.into(),
        base_state: d.state_id,
        base_epoch: 0,
        seed: None,
        digest: "4".repeat(64),
        operations: vec![Op::LayerMask(op.clone())],
    };
    let budget = SharedBudget::new(1 << 20);
    let mut first = stage(&d, &batch, &budget, SELECTION_FILTER_WORK_MAX);
    first.edit_layer_mask(&op).unwrap();
    let spent = SELECTION_FILTER_WORK_MAX - first.coverage_work;
    assert!(spent > 0);
    drop(first);
    assert_eq!(budget.usage().reserved, 0);
    // Give exactly one operation's measured planning allowance. The second
    // action must consume its remaining transaction work, not restart a limit.
    let mut both = stage(&d, &batch, &budget, spent);
    both.edit_layer_mask(&op).unwrap();
    assert_eq!(both.coverage_work, 0);
    both.operation = 1;
    assert_eq!(
        both.edit_layer_mask(&op).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    drop(both);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(
        d.layers[&id].mask.as_ref().unwrap().coverage().stats().runs,
        0
    );
}
#[test]
fn weighted_dense_selection_capture_shares_one_allowance_across_filter_and_layer_families() {
    use paintcraft_core::types::{filters::Descriptor, text::MaskSpan};
    use paintcraft_document::{FilterInstance, FilterStack};
    use paintcraft_raster::selection::BinaryMask;
    let mut d =
        paintcraft_document::new_document(64, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    let id = d.roots[0];
    let fid = Uuid::from_u128(71);
    d.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        fid,
        false,
        Descriptor::InvertRgbV1,
    )])
    .unwrap();
    let source = SharedBudget::new(1 << 20);
    let selected = BinaryMask::from_spans(
        Rect::new(0, 0, 64, 1).unwrap(),
        false,
        &[MaskSpan {
            y: 0,
            x0: 0,
            x1: 0,
            coverage: 255,
        }],
        &source,
        Default::default(),
        &ok,
    )
    .unwrap();
    let own = LayerMaskOp::FromSelection {
        layer: LayerRef::Id(id),
        enabled: false,
    };
    let batch = Batch {
        version: 7,
        policy: POLICY.into(),
        base_state: d.state_id,
        base_epoch: 0,
        seed: None,
        digest: "5".repeat(64),
        operations: vec![Op::LayerMask(own.clone())],
    };
    let budget = SharedBudget::new(1 << 20);
    let filter = paintcraft_commands::FilterRef::Id(fid);
    let action = paintcraft_commands::MaskOp::FromSelection { enabled: false };
    let mut measured = stage(&d, &batch, &budget, SELECTION_FILTER_WORK_MAX);
    measured.selection = Selection::Mask(selected.clone());
    measured.edit_filter_mask(id, &filter, &action).unwrap();
    measured.operation = 1;
    measured.edit_layer_mask(&own).unwrap();
    let total = SELECTION_FILTER_WORK_MAX - measured.coverage_work;
    assert!(total > 130);
    drop(measured);
    assert_eq!(budget.usage().reserved, 0);
    // Each dense64 row has65 weighted units in addition to callbacks. Remove
    // ONE of those charges: callback-only bookkeeping used to admit both.
    let mut denied = stage(&d, &batch, &budget, total - 65);
    denied.selection = Selection::Mask(selected.clone());
    denied.edit_filter_mask(id, &filter, &action).unwrap();
    denied.operation = 1;
    assert_eq!(
        denied.edit_layer_mask(&own).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    drop(denied);
    assert_eq!(budget.usage().reserved, 0);
    let mut exact = stage(&d, &batch, &budget, total);
    exact.selection = Selection::Mask(selected);
    exact.edit_filter_mask(id, &filter, &action).unwrap();
    exact.operation = 1;
    exact.edit_layer_mask(&own).unwrap();
    assert_eq!(exact.coverage_work, 0);
    assert_eq!(
        exact.doc.layers[&id]
            .mask
            .as_ref()
            .unwrap()
            .coverage()
            .coverage(0, 0),
        255
    );
    assert_eq!(
        exact.doc.layers[&id].filters.as_slice()[0]
            .mask()
            .unwrap()
            .coverage()
            .coverage(1, 0),
        0
    );
    drop(exact);
    assert_eq!(budget.usage().reserved, 0);
    assert!(d.layers[&id].mask.is_none());
    assert!(d.layers[&id].filters.as_slice()[0].mask().is_none());
}
