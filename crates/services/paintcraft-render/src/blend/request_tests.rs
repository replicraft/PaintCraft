//! Genuine public render reentry while its outer callback holds the CPU context.
use super::*;
use crate::spatial::{WorkspaceAdmission, with_workspace};
use paintcraft_core::types::{
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
#[test]
fn public_same_document_larger_roi_and_tighter_parent_recheck_before_render_body() {
    let mut d = paintcraft_document::new_document(
        64,
        64,
        Some(Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let mut front = d.layers[&d.roots[0]].clone();
    front.id = deterministic_id(&["reentry-front"]);
    front.blend_mode = Mode::Multiply;
    front.opacity = 64;
    front.raster = paintcraft_raster::RasterLayer::new(64, 64, Rgba([204, 77, 13, 128]));
    d.roots.push(front.id);
    d.layers.insert(front.id, front);
    let budget = SharedBudget::new(4 << 20);
    let scope = budget.scope(AllocationClass::SharedBudget);
    let a = WorkspaceAdmission::new(scope.clone(), 1 << 20, 544);
    let once = Cell::new(false);
    let nested = Cell::new(0);
    let out = with_workspace(a.clone(), || {
        crate::render_region_cancellable(&d, Rect::new(0, 0, 1, 1).unwrap(), || {
            if !once.replace(true) {
                assert!(allowed(&d));
                assert_eq!(a.work_used(), 512);
                for (region, tighten) in [
                    (Rect::new(0, 0, 2, 1).unwrap(), false),
                    (Rect::new(0, 0, 1, 1).unwrap(), true),
                ] {
                    let before = a.work_used();
                    let checks = Cell::new(0);
                    let run = || {
                        crate::render_region_cancellable(&d, region, || {
                            checks.set(checks.get() + 1);
                            Ok(())
                        })
                    };
                    let result = if tighten {
                        let current = crate::spatial::current().unwrap();
                        let child = current.tightened(current.bytes_limit(), 543)?;
                        assert!(child.same_job(&a));
                        with_workspace(child, run)
                    } else {
                        run()
                    };
                    assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
                    // Only quote_document_levels' two root-inventory callbacks run.
                    // The next callback would be the render body's initial check,
                    // before output_buffer. Old allowed() bypass reached that body.
                    assert_eq!(checks.get(), 2);
                    assert_eq!(a.work_used(), before);
                    assert!(allowed(&d));
                    nested.set(nested.get() + 1);
                }
            }
            Ok(())
        })
    })
    .unwrap();
    assert_eq!(out, [249, 233, 225, 255]);
    assert_eq!(nested.get(), 2);
    assert_eq!(a.work_used(), 544);
    assert!(!allowed(&d));
    assert_eq!(scope.footprint().live, 0);
}
