//! Concrete Source7 owner paths; public standalone Stage retirement stays separate.
use super::*;
use paintcraft_core::types::resources::{BudgetScope, BudgetWatch};
use paintcraft_raster::{Admission, TileVersion};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
fn document() -> (Document, Uuid) {
    let mut d = fixture(128, true);
    let root = d.roots[0];
    let id = deterministic_id(&["compact owner filter"]);
    let l = d.layers.get_mut(&root).unwrap();
    l.raster
        .blend_pixel(
            0,
            0,
            Rgba([7, 8, 9, 128]),
            paintcraft_raster::BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    l.filters = FilterStack::new(vec![
        FilterInstance::new(id, false, Descriptor::InvertRgbV1).with_mask(Some(FilterMask::new(
            true,
            l.mask.as_ref().unwrap().coverage().clone(),
        ))),
    ])
    .unwrap();
    (d, id)
}
fn duplicate(s: &LayerAppearanceService, id: Uuid, generation: u64, render: bool) -> Value {
    let op = Value::object([
        ("op", Value::str("filter.duplicate")),
        (
            "layer_ref",
            Value::object([("id", Value::str(s.state.base.roots[0].to_string()))]),
        ),
        (
            "filter_ref",
            Value::object([("id", Value::str(id.to_string()))]),
        ),
        ("ref", Value::str("copy")),
    ]);
    request(s, generation, render, op)
}
fn version(s: &LayerAppearanceService) -> TileVersion {
    s.state.base.layers[&s.state.base.roots[0]]
        .raster
        .tile_leases()
        .next()
        .unwrap()
        .1
        .version()
}
fn observe_metadata_drop(
    s: &LayerAppearanceService,
    version: TileVersion,
) -> (BudgetWatch, Arc<AtomicBool>, BudgetScope) {
    let metadata = s.state.metadata_stage.clone();
    assert!(metadata.footprint().live > 0);
    let scope = metadata.clone();
    let seen = Arc::new(AtomicBool::new(false));
    let observed = seen.clone();
    let watch = metadata
        .budget()
        .watch(1, move || {
            if scope.footprint().live == 0 {
                observed.store(version.try_lease().is_some(), Ordering::Release);
            }
        })
        .unwrap();
    watch.observe();
    watch.wait_for_release(true);
    (watch, seen, metadata)
}
#[test]
fn whole_service_teardown_releases_candidate_account_while_base_tile_is_live() {
    let (d, id) = document();
    let mut s = open(&d, false);
    let input = duplicate(&s, id, 1, false);
    s.prepare(&text(&input)).unwrap();
    let version = version(&s);
    assert!(s.state.candidate.is_some());
    let budget = s.state.metadata_stage.budget();
    let (watch, seen, scope) = observe_metadata_drop(&s, version.clone());
    drop(s);
    assert!(
        seen.load(Ordering::Acquire),
        "candidate account must release before base's final raw tile"
    );
    assert_eq!(scope.footprint().live, 0);
    assert!(version.try_lease().is_none());
    assert_eq!(budget.usage().reserved, 0);
    drop(watch);
}
#[test]
fn cancel_replace_and_prepare_error_preserve_same_base_and_cumulative_owner() {
    let (d, id) = document();
    // Captured jobs debit their original parent immediately. Standalone
    // render:false quotes instead retain deferred execution work.
    let (bytes, control) = source(&d, admission(false));
    let budget = SharedBudget::new(64 << 20);
    let parent = paintcraft_render::spatial::WorkspaceAdmission::new(
        budget.scope(AllocationClass::SharedBudget),
        64 << 20,
        268435456,
    );
    let mut s = paintcraft_render::spatial::with_workspace(parent.clone(), || {
        LayerAppearanceService::open(&bytes, &text(&control))
    })
    .unwrap();
    let version = version(&s);
    let first = duplicate(&s, id, 1, false);
    s.prepare(&text(&first)).unwrap();
    let used = s.state.job.used().unwrap().base;
    let error = Error::new(ErrorCode::OperationCancelled, "before Source7 replacement").at(19);
    let second = duplicate(&s, id, 2, false);
    assert_eq!(
        s.prepare_checked(&text(&second), &|| Err(error.clone()))
            .unwrap_err(),
        error
    );
    assert!(s.state.candidate.is_some());
    assert!(version.try_lease().is_some());
    s.prepare(&text(&second)).unwrap();
    assert!(s.state.job.used().unwrap().base > used);
    assert_eq!(s.state.job.used().unwrap().base, parent.work_used());
    let candidate = s.state.candidate.as_ref().unwrap();
    assert_eq!(
        candidate.document().layers[&s.state.base.roots[0]]
            .raster
            .tile_leases()
            .next()
            .unwrap()
            .1
            .version(),
        version
    );
    let (watch, seen, scope) = observe_metadata_drop(&s, version.clone());
    s.cancel(2).unwrap();
    assert!(seen.load(Ordering::Acquire));
    assert!(s.state.candidate.is_none());
    assert_eq!(scope.footprint().live, 0);
    assert!(version.try_lease().is_some());
    let after = s.state.job.used().unwrap().base;
    assert!(after >= used);
    assert_eq!(after, parent.work_used());
    drop(watch);
    drop(s);
    assert!(version.try_lease().is_none());
}
#[test]
fn escaped_pixel_frame_retains_accounts_but_never_raw_candidate_or_base_tiles() {
    let (d, id) = document();
    let mut s = open(&d, false);
    let input = duplicate(&s, id, 1, true);
    s.prepare(&text(&input)).unwrap();
    finish(&mut s);
    let version = version(&s);
    let packet = s.take_frame().unwrap();
    let budget = packet._output.budget();
    let (watch, seen, scope) = observe_metadata_drop(&s, version.clone());
    drop(s);
    assert!(seen.load(Ordering::Acquire));
    assert_eq!(scope.footprint().live, 0);
    assert!(
        version.try_lease().is_none(),
        "escaped frame holds pixels/programs/accounts, no raw document aliases"
    );
    assert_eq!(packet.pixels(), [7, 8, 9, 32, 20, 115, 200, 32]);
    assert!(budget.usage().reserved > 0);
    drop(packet);
    assert_eq!(budget.usage().reserved, 0);
    drop(watch);
}
