use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{
    Rect, Rgba,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::session::Session;
use paintcraft_raster::{Admission, RasterLayer};
use std::{cell::RefCell, sync::Arc};

#[test]
fn metadata_interference_after_a_private_edit_preserves_owner_and_releases_all_attempt_storage() {
    let doc = new_document(8, 8, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
    let id = doc.roots[0];
    let batch = Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: doc.state_id,
        base_epoch: 0,
        seed: None,
        digest: "0".repeat(64),
        operations: (0..2)
            .map(|x| Op::FillRect {
                layer: LayerRef::Id(id),
                rect: Rect::new(x, 0, 1, 1).unwrap(),
                color: Rgba([17, 88, 156, 128]),
                mode: Mode::SourceOver,
            })
            .collect(),
    };
    let session = Session::new(doc);
    let snapshot = session.snapshot();
    let ticket = session.ticket();
    let history = session.history_view();
    let decoded = SharedBudget::new(2 * RasterLayer::TILE_BYTES as u64);
    let metadata = SharedBudget::new(1 << 20);
    let reservation = decoded
        .reserve_capacity(2 * RasterLayer::TILE_BYTES as u64)
        .unwrap();
    let probe = reservation.clone();
    let blockers = RefCell::new(Vec::new());
    let error = session
        .preparation()
        .unwrap()
        .apply_with_admission(
            &batch,
            Admission::with_reserved_residency(2 * 65_536, reservation)
                .with_paint_metadata_budget(metadata.clone()),
            || {
                if probe.available() < probe.capacity() {
                    let usage = metadata.usage();
                    let free = usage.limit - usage.reserved;
                    if free != 0 {
                        blockers.borrow_mut().push(metadata.reserve(free).unwrap());
                    }
                }
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(error.operation_index, Some(1));
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        AllocationClass::PaintMetadata
    );
    assert_eq!(probe.available(), probe.capacity());
    drop(probe);
    assert_eq!(decoded.usage().reserved, 0);
    assert!(metadata.usage().reserved > 0);
    assert_eq!(session.ticket(), ticket);
    assert_eq!(session.history_view(), history);
    assert!(Arc::ptr_eq(&snapshot, &session.snapshot()));
    drop(blockers);
    assert_eq!(metadata.usage().reserved, 0);
}
