use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba, resources::SharedBudget};
use paintcraft_raster::{Admission, BlendMode, CropPlan, PaintSnapshot, RasterLayer};
use std::cell::Cell;

fn source() -> RasterLayer {
    let mut raster = RasterLayer::new(520, 270, Rgba([7, 8, 9, 0]));
    let mut admission = Admission::new(4 * 65536);
    for (x, y, pixel) in [
        (255, 255, Rgba([31, 63, 127, 0])),
        (256, 256, Rgba([200, 80, 20, 128])),
        (300, 260, Rgba([30, 40, 50, 255])),
    ] {
        raster
            .blend_pixel(x, y, pixel, BlendMode::Replace, &mut admission)
            .unwrap();
    }
    raster
}

#[test]
fn unaligned_crop_is_exact_and_preserves_transparent_rgb_without_preflight_decoding() {
    let original = source();
    let area = Rect::new(250, 250, 60, 18).unwrap();
    let metadata_admission = Admission::with_residency(4 * 65536, SharedBudget::new(0));
    let snapshot = PaintSnapshot::capture(&original, &metadata_admission, &|| Ok(())).unwrap();
    let crop = CropPlan::prepare(snapshot, area, &metadata_admission, &|| Ok(())).unwrap();
    assert_eq!(metadata_admission.residency_usage().peak, 0);
    assert_eq!(
        crop.clone()
            .replay(&mut metadata_admission.clone(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let output = crop
        .replay(&mut Admission::new(4 * 65536), &|| Ok(()))
        .unwrap();
    assert_eq!((output.width(), output.height()), (60, 18));
    assert_eq!(output.default_pixel(), Rgba([7, 8, 9, 0]));
    for y in 0..18 {
        for x in 0..60 {
            assert_eq!(output.get(x, y), original.get(x + 250, y + 250));
        }
    }
    assert_eq!(output.get(5, 5), Rgba([31, 63, 127, 0]));
    assert_eq!(original.get(255, 255), Rgba([31, 63, 127, 0]));
}

#[test]
fn nested_crops_and_expand_read_the_frozen_logical_prefix_without_resampling_or_decoding() {
    let original = source();
    let admission = Admission::with_residency(8 * 65536, SharedBudget::new(0));
    let mut prefix = admission.paint_plan();
    prefix
        .rect(
            Rect::new(258, 252, 8, 8).unwrap(),
            Rgba([150, 50, 20, 128]),
            BlendMode::SourceOver,
        )
        .unwrap();
    let input = PaintSnapshot::capture(&original, &admission, &|| Ok(()))
        .unwrap()
        .fork(&admission, &|| Ok(()))
        .unwrap()
        .append_checked(prefix, &|| Ok(()))
        .unwrap()
        .freeze_checked(&admission, &|| Ok(()))
        .unwrap();
    let first = CropPlan::prepare(
        input,
        Rect::new(250, 250, 30, 18).unwrap(),
        &admission,
        &|| Ok(()),
    )
    .unwrap();
    let second = CropPlan::prepare(
        first.snapshot(&admission, &|| Ok(())).unwrap(),
        Rect::new(5, 1, 20, 14).unwrap(),
        &admission,
        &|| Ok(()),
    )
    .unwrap();
    let virtual_source = second.snapshot(&admission, &|| Ok(())).unwrap();
    let reader = virtual_source.reader(&admission, &|| Ok(())).unwrap();
    for y in 0..14 {
        for x in 0..20 {
            let mut expected = original.get(x + 255, y + 251);
            if (258..266).contains(&(x + 255)) && (252..260).contains(&(y + 251)) {
                expected = Rgba([150, 50, 20, 128]).source_over(expected);
            }
            assert_eq!(
                reader.read_checked(x, y, &|| Ok(())).unwrap(),
                expected,
                "{x},{y}"
            );
        }
    }
    let expanded = virtual_source
        .fork_expanded(100, 100, &admission, &|| Ok(()))
        .unwrap()
        .freeze_checked(&admission, &|| Ok(()))
        .unwrap();
    let reader = expanded.reader(&admission, &|| Ok(())).unwrap();
    assert_eq!(
        reader.read_checked(40, 40, &|| Ok(())).unwrap(),
        original.default_pixel()
    );
    assert_eq!(admission.residency_usage().peak, 0);
    let output = second
        .replay(&mut Admission::new(8 * 65536), &|| Ok(()))
        .unwrap();
    for y in 0..14 {
        for x in 0..20 {
            assert_eq!(
                output.get(x, y),
                reader.read_checked(x, y, &|| Ok(())).unwrap()
            );
        }
    }
}

#[test]
fn huge_uniform_crops_keep_implicit_pixels_and_need_no_decoded_tiles() {
    let default = Rgba([37, 19, 83, 0]);
    let source = RasterLayer::new(1_000_000, 1_000_000, default);
    let mut admission = Admission::with_residency(0, SharedBudget::new(0));
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() > 64 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "uniform crop iterated pixels",
            ))
        } else {
            Ok(())
        }
    };
    let snapshot = PaintSnapshot::capture(&source, &admission, &check).unwrap();
    let plan = CropPlan::prepare(
        snapshot,
        Rect::new(17, 23, 900_000, 900_000).unwrap(),
        &admission,
        &check,
    )
    .unwrap();
    assert_eq!(plan.tile_coords().count(), 0);
    let output = plan.replay(&mut admission, &check).unwrap();
    assert_eq!(output.resident_tiles(), 0);
    assert_eq!(output.get(800_000, 800_000), default);
}

#[test]
fn cancellation_and_invalid_crop_leave_the_source_unchanged() {
    let original = source();
    let admission = Admission::new(4 * 65536);
    let snapshot = PaintSnapshot::capture(&original, &admission, &|| Ok(())).unwrap();
    assert_eq!(
        CropPlan::prepare(
            snapshot.clone(),
            Rect::new(-1, 0, 20, 20).unwrap(),
            &admission,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
    let calls = Cell::new(0);
    let error = CropPlan::prepare(
        snapshot,
        Rect::new(250, 250, 30, 18).unwrap(),
        &admission,
        &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 40 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel crop read",
                ))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(original.get(256, 256), Rgba([200, 80, 20, 128]));
}
