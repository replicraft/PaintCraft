use super::*;
use crate::blend::Mode;
use crate::spatial::{WorkspaceAdmission, with_workspace};
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use paintcraft_core::types::{Error, ErrorCode};
use std::cell::Cell;

#[test]
fn shared_row_keeps_old_general_debit_and_seed_has_no_per_pixel_blend_result() {
    let doc = paintcraft_document::new_document(
        4,
        1,
        Some(Rgba([200, 30, 80, 128])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let mut layer = doc.layers[&doc.roots[0]].clone();
    layer.opacity = 128;
    assert!(std::mem::size_of::<Option<Kind>>() <= std::mem::size_of::<Kind>());
    for mode in [Mode::Multiply, Mode::Screen] {
        for kind in [Some(Kind::General(mode)), None] {
            let budget = SharedBudget::new(1 << 20);
            let scope = budget
                .reserve_capacity(1 << 20)
                .unwrap()
                .scope(AllocationClass::SharedBudget);
            let base = WorkspaceAdmission::new(scope.clone(), 1 << 20, 1 << 20);
            let checks = Cell::new(0);
            let mut out = [0u8; 16];
            with_workspace(base.clone(), || {
                row_shared(&layer, 0, 0, &mut out, kind, &|| {
                    checks.set(checks.get() + 1);
                    Ok(())
                })
            })
            .unwrap();
            assert_eq!(out.as_slice(), [200, 30, 80, 64].repeat(4));
            assert_eq!(checks.get(), 1);
            assert_eq!(base.work_used(), if kind.is_some() { 1024 } else { 0 });
            assert_eq!(scope.footprint().live, 0);
        }
    }
}

#[test]
fn shared_chunk_cancellation_keeps_original_error_output_and_work_prefix() {
    let doc = paintcraft_document::new_document(
        4,
        1,
        Some(Rgba([7, 9, 11, 0])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let layer = &doc.layers[&doc.roots[0]];
    for kind in [Some(Kind::General(Mode::Multiply)), None] {
        let budget = SharedBudget::new(1 << 20);
        let scope = budget
            .reserve_capacity(1 << 20)
            .unwrap()
            .scope(AllocationClass::SharedBudget);
        let base = WorkspaceAdmission::new(scope.clone(), 1 << 20, 1 << 20);
        let mut out = [42u8; 16];
        let error = with_workspace(base.clone(), || {
            row_shared(layer, 0, 0, &mut out, kind, &|| {
                Err(Error::new(ErrorCode::OperationCancelled, "shared row original check").at(19))
            })
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.message, "shared row original check");
        assert_eq!(error.operation_index, Some(19));
        assert_eq!(out, [42; 16]);
        assert_eq!(base.work_used(), 0);
    }
}
