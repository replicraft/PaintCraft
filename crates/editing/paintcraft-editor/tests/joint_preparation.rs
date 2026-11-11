//! Active batch executor: all compiled paint metadata precedes decoded replay.
use paintcraft_commands::{Batch, LayerRef, Mode, Op};
use paintcraft_core::types::{
    Rect, Rgba,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::execute::{Selection, execute_with_admission, execute_with_geometry_cache};
use paintcraft_raster::{Admission, RasterLayer};
use std::cell::RefCell;
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
fn batch(doc: &paintcraft_document::Document, operations: Vec<Op>) -> Batch {
    Batch {
        version: 2,
        policy: POLICY.into(),
        base_state: doc.state_id,
        base_epoch: 0,
        seed: None,
        digest: "0".repeat(64),
        operations,
    }
}
#[test]
fn metadata_pressure_after_complete_decoded_reservation_cannot_require_another_paint_index() {
    let doc = new_document(257, 1, Some(Rgba([7, 9, 11, 0])), 0, POLICY).unwrap();
    let layer = LayerRef::Id(doc.roots[0]);
    let request = batch(
        &doc,
        vec![
            Op::FillRect {
                layer: layer.clone(),
                rect: Rect::new(0, 0, 1, 1).unwrap(),
                color: Rgba([20, 60, 100, 128]),
                mode: Mode::SourceOver,
            },
            Op::FillRect {
                layer,
                rect: Rect::new(256, 0, 1, 1).unwrap(),
                color: Rgba([8, 16, 32, 255]),
                mode: Mode::Replace,
            },
        ],
    );
    let metadata = SharedBudget::new(1 << 20);
    let decoded = SharedBudget::new(2 * BYTES);
    let blocker = RefCell::new(None);
    let result = execute_with_geometry_cache(
        &doc,
        Selection::Inactive,
        &request,
        Admission::with_residency(2 * 65_536, decoded.clone())
            .with_paint_metadata_budget(metadata.clone()),
        None,
        &|| {
            if decoded.usage().reserved != 0 && blocker.borrow().is_none() {
                // Pressure injection only, after the original 1MiB policy and
                // complete prepared owners exist. This does not size a grant.
                let held = metadata.reserve((1 << 20) - metadata.usage().reserved)?;
                *blocker.borrow_mut() = Some(held);
            }
            Ok(())
        },
    )
    .unwrap();
    assert!(blocker.borrow().is_some());
    assert_eq!(
        result.doc.layers[&doc.roots[0]].raster.get(0, 0),
        Rgba([20, 60, 100, 128])
    );
    assert_eq!(
        result.doc.layers[&doc.roots[0]].raster.get(256, 0),
        Rgba([8, 16, 32, 255])
    );
    assert_eq!(
        doc.layers[&doc.roots[0]].raster.get(0, 0),
        Rgba([7, 9, 11, 0])
    );
    drop(result);
    drop(blocker);
    assert_eq!(metadata.usage().reserved, 0);
    assert_eq!(decoded.usage().reserved, 0);
}
#[test]
fn decoded_denial_retires_earlier_paint_flood_scratch_and_all_prepared_indices() {
    let doc = new_document(513, 1, Some(Rgba([7, 9, 11, 0])), 0, POLICY).unwrap();
    let layer = LayerRef::Id(doc.roots[0]);
    let request = batch(
        &doc,
        vec![
            Op::FillRect {
                layer: layer.clone(),
                rect: Rect::new(0, 0, 1, 1).unwrap(),
                color: Rgba([255; 4]),
                mode: Mode::Replace,
            },
            Op::FloodFill {
                layer,
                at: (256, 0),
                color: Rgba([20, 60, 100, 128]),
                mode: Mode::Replace,
                tolerance: 0,
            },
        ],
    );
    let metadata = SharedBudget::new(1 << 20);
    let decoded = SharedBudget::new(BYTES);
    let before = doc.clone();
    let error = execute_with_admission(
        &doc,
        Selection::Inactive,
        &request,
        Admission::with_residency(3 * 65_536, decoded.clone())
            .with_paint_metadata_budget(metadata.clone()),
    )
    .unwrap_err();
    let denial = error.allocation_denial().unwrap();
    assert_eq!(denial.class(), AllocationClass::DecodedTiles);
    assert_eq!(denial.required_bytes(), 3 * BYTES);
    assert_eq!(error.operation_index, Some(1));
    assert_eq!(decoded.usage().peak, 0);
    assert_eq!(decoded.usage().reserved, 0);
    assert!(metadata.usage().peak > 0);
    assert_eq!(metadata.usage().reserved, 0);
    assert!(doc.same_semantics(&before));
}

#[test]
fn one_hundred_translucent_diagonals_fit_original_observed_204800_byte_budget() {
    let doc = new_document(257, 257, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    let op = Op::PencilPath {
        layer: LayerRef::Id(doc.roots[0]),
        points: vec![(0, 0), (256, 256)],
        color: Rgba([17, 88, 156, 19]),
        mode: Mode::SourceOver,
    };
    let metadata = SharedBudget::new(204800);
    let decoded = SharedBudget::new(4 * BYTES);
    let result = execute_with_admission(
        &doc,
        Selection::Inactive,
        &batch(&doc, vec![op; 100]),
        Admission::with_residency(4 * 65_536, decoded.clone())
            .with_paint_metadata_budget(metadata.clone()),
    )
    .unwrap();
    // Independent per-channel repeated source-over against fully opaque white.
    let mut expected = Rgba([255; 4]);
    for _ in 0..100 {
        for (out, source) in expected.0[..3].iter_mut().zip([17u32, 88, 156]) {
            *out = ((source * 19 + u32::from(*out) * 236 + 127) / 255) as u8;
        }
    }
    let raster = &result.doc.layers[&doc.roots[0]].raster;
    for p in [(0, 0), (128, 128), (256, 256)] {
        assert_eq!(raster.get(p.0, p.1), expected);
    }
    assert_eq!(raster.get(1, 0), Rgba([255; 4]));
    assert_eq!(doc.layers[&doc.roots[0]].raster.get(0, 0), Rgba([255; 4]));
    assert!(metadata.usage().peak <= 204800);
    assert_eq!(metadata.usage().reserved, 0);
    drop(result);
    assert_eq!(decoded.usage().reserved, 0);
}
