use paintcraft_core::types::{ErrorCode, Rect, Rgba, resources::SharedBudget};
use paintcraft_raster::{Admission, BlendMode, PaintPlan, RasterLayer};
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
fn dot(x: i64) -> Rect {
    Rect::new(x, 1, 1, 1).unwrap()
}
fn metadata_error(error: paintcraft_core::types::Error) {
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(error.message, "paint plan metadata budget exceeded");
    assert!(error.allocation_denial().is_none());
}

#[test]
fn descriptors_have_a_fixed_capacity_limit_and_coalescing_uses_no_new_capacity() {
    let mut none = PaintPlan::with_metadata_limit(0);
    metadata_error(
        none.rect(dot(0), Rgba([1, 2, 3, 255]), BlendMode::Replace)
            .unwrap_err(),
    );
    assert_eq!(none.descriptor_bytes(), 0);
    let mut plan = PaintPlan::with_metadata_limit(256);
    let color = Rgba([17, 88, 156, 128]);
    plan.rect(dot(0), color, BlendMode::SourceOver).unwrap();
    let initial = plan.descriptor_bytes();
    for x in 1..1000 {
        plan.rect(dot(x), color, BlendMode::SourceOver).unwrap();
    }
    assert_eq!(plan.descriptor_bytes(), initial);
    let mut rejected = false;
    for _ in 0..1000 {
        if let Err(error) = plan.rect(dot(0), color, BlendMode::SourceOver) {
            metadata_error(error);
            rejected = true;
            break;
        }
        assert!(plan.descriptor_bytes() <= 256);
    }
    assert!(rejected);
    assert!(plan.descriptor_bytes() <= 256);
}

#[test]
fn repeated_write_index_is_bounded_even_with_one_touched_tile() {
    let mut plan = PaintPlan::default();
    let color = Rgba([17, 88, 156, 128]);
    for _ in 0..1024 {
        plan.rect(dot(1), color, BlendMode::SourceOver).unwrap();
    }
    let budget = SharedBudget::new(BYTES);
    let mut layer = RasterLayer::new(8, 8, Rgba([31, 63, 127, 0]));
    let mut admission = Admission::with_residency(65_536, budget.clone())
        .with_paint_metadata_limit(plan.descriptor_bytes() + 512);
    metadata_error(plan.apply(&mut layer, &mut admission).unwrap_err());
    assert_eq!(budget.usage().peak, 0);
    assert_eq!(layer.resident_tiles(), 0);
    assert_eq!(layer.get(1, 1), Rgba([31, 63, 127, 0]));
    let mut reference = Rgba([31, 63, 127, 0]);
    for _ in 0..1024 {
        reference = color.source_over(reference);
    }
    plan.apply(
        &mut layer,
        &mut Admission::with_residency(65_536, budget.clone()),
    )
    .unwrap();
    assert_eq!(layer.get(1, 1), reference);
    assert_eq!(layer.get(0, 1), Rgba([31, 63, 127, 0]));
    assert_eq!(budget.usage().reserved, BYTES);
    drop(layer);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn aggregate_tile_table_and_vectors_fail_before_any_decoded_tile_prefix() {
    let mut plan = PaintPlan::default();
    plan.rect(
        Rect::new(0, 0, 4096, 4096).unwrap(),
        Rgba([1, 2, 3, 255]),
        BlendMode::Replace,
    )
    .unwrap();
    let budget = SharedBudget::new(256 * BYTES);
    let mut layer = RasterLayer::new(4096, 4096, Rgba::TRANSPARENT);
    let mut admission = Admission::with_residency(256 * 65_536, budget.clone())
        .with_paint_metadata_limit(plan.descriptor_bytes() + 1024);
    metadata_error(plan.apply(&mut layer, &mut admission).unwrap_err());
    assert_eq!(layer.resident_tiles(), 0);
    assert_eq!(budget.usage().peak, 0);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn no_op_and_existing_index_entries_do_not_require_speculative_table_growth() {
    let mut plan = PaintPlan::default();
    let color = Rgba([1, 2, 3, 255]);
    // Fill the implementation's initial map, then revisit an existing bucket.
    let mut probe = std::collections::HashMap::<paintcraft_raster::TileCoord, Vec<usize>>::new();
    probe.try_reserve(1).unwrap();
    let entries = probe.capacity();
    for i in 0..entries {
        plan.rect(dot((i * 256) as i64), color, BlendMode::Replace)
            .unwrap();
    }
    plan.rect(dot(0), color, BlendMode::Replace).unwrap();
    let mut layer = RasterLayer::new((entries * 256) as u32, 8, Rgba::TRANSPARENT);
    let mut admission = Admission::new(entries as u64 * 65_536).with_paint_metadata_limit(
        plan.descriptor_bytes()
            + entries
                * (std::mem::size_of::<(paintcraft_raster::TileCoord, Vec<usize>)>()
                    + 4 * std::mem::size_of::<usize>()),
    );
    plan.apply(&mut layer, &mut admission).unwrap();
    assert_eq!(layer.resident_tiles(), entries);
    let mut no_op = PaintPlan::default();
    no_op
        .rect(dot(1), Rgba([31, 63, 127, 0]), BlendMode::SourceOver)
        .unwrap();
    let mut empty = RasterLayer::new(8, 8, Rgba([7, 9, 11, 0]));
    let mut no_index = Admission::new(0).with_paint_metadata_limit(no_op.descriptor_bytes());
    no_op.apply(&mut empty, &mut no_index).unwrap();
    assert_eq!(empty.resident_tiles(), 0);
    assert_eq!(empty.get(1, 1), Rgba([7, 9, 11, 0]));
}
