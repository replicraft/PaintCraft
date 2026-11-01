//! Golden comparison against the existing portable encoder, never native Zstd.
use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use paintcraft_document::LayerMask;
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use std::cell::Cell;
fn document() -> Document {
    let bytes: Vec<u8> = (0..257 * 3)
        .flat_map(|i| {
            [
                i as u8,
                (i * 17) as u8,
                91,
                if i % 2 == 0 { 0 } else { 255 },
            ]
        })
        .collect();
    let mut doc = paintcraft_document::import_image(257, 3, &bytes, "admitted").unwrap();
    let mask = A8Mask::uniform(
        paintcraft_core::types::Rect::from_origin_size(-1, 0, 259, 3).unwrap(),
        127,
        &SharedBudget::new(4096),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().mask = Some(LayerMask::new(false, mask));
    doc
}
fn encode(doc: &Document, budget: u64, check: &dyn Fn() -> Result<()>) -> Result<(Vec<u8>, u64)> {
    let realm = SharedBudget::new(budget);
    let scope = realm
        .reserve_capacity(budget)?
        .scope(AllocationClass::PaintMetadata);
    let mut bytes = Vec::new();
    write_portable_snapshot_admitted_checked(doc, &mut bytes, &scope, check, &|_| Ok(()))?;
    assert_eq!(scope.footprint().live, 0);
    Ok((bytes, scope.footprint().peak))
}
#[test]
fn portable_bytes_match_existing_writer_with_hidden_rgb_finite_edges_and_mask() {
    let doc = document();
    let mut original = Vec::new();
    super::super::write_encoded(
        &doc,
        Output::new(&mut original),
        &|| Ok(()),
        compression::Encoder::portable(),
    )
    .unwrap();
    let (actual, _) = encode(
        &doc,
        portable_snapshot_scratch_quote(&doc).unwrap(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(actual, original);
    let source = doc.layers[&doc.roots[0]].raster.get(0, 0);
    assert_eq!(source.0, [0, 0, 91, 0]);
}
#[test]
fn actual_peak_minus_one_refuses_without_borrowing_global_spare_capacity() {
    let doc = document();
    let quote = portable_snapshot_scratch_quote(&doc).unwrap();
    let (_, peak) = encode(&doc, quote, &|| Ok(())).unwrap();
    let global = SharedBudget::new(quote * 2);
    let scope = global
        .reserve_capacity(peak - 1)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let mut bytes = Vec::new();
    assert_eq!(
        write_portable_snapshot_admitted_checked(&doc, &mut bytes, &scope, &|| Ok(()), &|_| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(scope.footprint().live, 0);
}
#[test]
fn cancellation_and_work_refusal_keep_source_and_release_all_scratch() {
    let doc = document();
    let before = crate::pcnd::write(&doc).unwrap();
    let count = Cell::new(0);
    let cancelled = || {
        count.set(count.get() + 1);
        if count.get() == 7 {
            Err(Error::new(ErrorCode::OperationCancelled, "stop"))
        } else {
            Ok(())
        }
    };
    assert_eq!(
        encode(
            &doc,
            portable_snapshot_scratch_quote(&doc).unwrap(),
            &cancelled
        )
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    let global = SharedBudget::new(1 << 24);
    let scope = global.scope(AllocationClass::PaintMetadata);
    let mut bytes = Vec::new();
    let error =
        write_portable_snapshot_admitted_checked(&doc, &mut bytes, &scope, &|| Ok(()), &|_| {
            Err(Error::new(ErrorCode::ResourceLimit, "work"))
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(bytes.is_empty());
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(crate::pcnd::write(&doc).unwrap(), before);
}
#[test]
fn multiple_aliased_and_zero_masks_keep_original_root_order_and_reject_duplicates() {
    use paintcraft_core::types::filters::Descriptor;
    use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
    let mut doc = document();
    let first = doc.roots[0];
    let coverage = doc.layers[&first].mask.as_ref().unwrap().coverage().clone();
    doc.layers.get_mut(&first).unwrap().filters = FilterStack::new(vec![
        FilterInstance::new(Uuid::from_u128(101), false, Descriptor::InvertRgbV1)
            .with_mask(Some(FilterMask::new(true, coverage))),
    ])
    .unwrap();
    let second = Uuid::from_u128(102);
    let mut cloned = doc.layers[&first].clone();
    cloned.id = second;
    let zero = A8Mask::uniform(
        paintcraft_core::types::Rect::from_origin_size(0, 0, 257, 3).unwrap(),
        0,
        &SharedBudget::new(4096),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    cloned.mask = Some(LayerMask::new(true, zero));
    doc.layers.insert(second, cloned);
    doc.roots.insert(0, second);
    let mut original = Vec::new();
    super::super::write_encoded(
        &doc,
        Output::new(&mut original),
        &|| Ok(()),
        compression::Encoder::portable(),
    )
    .unwrap();
    let (actual, _) = encode(
        &doc,
        portable_snapshot_scratch_quote(&doc).unwrap(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(actual, original);
    let before = doc.roots.clone();
    doc.roots[1] = second;
    assert_eq!(
        encode(
            &doc,
            portable_snapshot_scratch_quote(&doc).unwrap(),
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::CodecFailed
    );
    assert_eq!(doc.roots, vec![second, second]);
    doc.roots = before;
    assert_eq!(doc.roots, vec![second, first]);
    let empty =
        paintcraft_document::new_document(1, 1, None, 103, paintcraft_document::POLICY).unwrap();
    let mut original = Vec::new();
    super::super::write_encoded(
        &empty,
        Output::new(&mut original),
        &|| Ok(()),
        compression::Encoder::portable(),
    )
    .unwrap();
    assert_eq!(
        encode(
            &empty,
            portable_snapshot_scratch_quote(&empty).unwrap(),
            &|| Ok(())
        )
        .unwrap()
        .0,
        original
    );
}
