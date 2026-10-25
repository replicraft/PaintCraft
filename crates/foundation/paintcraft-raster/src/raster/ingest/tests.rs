use super::*;
use std::cell::Cell;
fn pixels(w: u32, h: u32) -> Vec<u8> {
    (0..h)
        .flat_map(|y| {
            (0..w).flat_map(move |x| {
                if x < 256 && y < 256 {
                    [0; 4]
                } else {
                    [x as u8, y as u8, 71, if (x + y) % 2 == 0 { 0 } else { 127 }]
                }
            })
        })
        .collect()
}
#[test]
fn exact_edges_sparse_defaults_hidden_rgb_and_versions_match_independent_pixels() {
    let (w, h) = (259, 258);
    let input = pixels(w, h);
    let budget = SharedBudget::new(3 * RasterLayer::TILE_BYTES as u64);
    let mut admission = Admission::with_residency(u64::MAX, budget.clone());
    let layer =
        RasterLayer::from_rgba_checked(w, h, Rgba::TRANSPARENT, &input, &mut admission, &|| Ok(()))
            .unwrap();
    assert_eq!(layer.resident_tiles(), 3);
    assert_eq!(budget.usage().reserved, 3 * RasterLayer::TILE_BYTES as u64);
    for y in 0..h {
        for x in 0..w {
            let index = ((y * w + x) * 4) as usize;
            assert_eq!(layer.get(x.into(), y.into()).0, &input[index..index + 4]);
        }
    }
    let versions: Vec<_> = layer
        .region_versions(Rect::new(0, 0, w.into(), h.into()).unwrap())
        .unwrap()
        .unwrap()
        .collect();
    assert!(!versions.is_empty());
    let clone = layer.clone();
    drop(layer);
    drop(admission);
    assert_eq!(budget.usage().reserved, 3 * RasterLayer::TILE_BYTES as u64);
    drop(clone);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn peak_denial_happens_before_any_tile_is_allocated() {
    let input = pixels(259, 258);
    let budget = SharedBudget::new(2 * RasterLayer::TILE_BYTES as u64);
    let mut admission = Admission::with_residency(u64::MAX, budget.clone());
    let failure = RasterLayer::from_rgba_checked(
        259,
        258,
        Rgba::TRANSPARENT,
        &input,
        &mut admission,
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(failure.code, ErrorCode::ResourceLimit);
    assert_eq!(budget.usage().peak, 0);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn every_cancellation_releases_the_entire_private_attempt() {
    let input = pixels(257, 2);
    let budget = SharedBudget::new(2 * RasterLayer::TILE_BYTES as u64);
    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    {
        let mut admission = Admission::with_residency(u64::MAX, budget.clone());
        drop(
            RasterLayer::from_rgba_checked(
                257,
                2,
                Rgba::TRANSPARENT,
                &input,
                &mut admission,
                &count,
            )
            .unwrap(),
        );
    }
    for stop in 1..=calls.get() {
        let current = Cell::new(0);
        let check = || {
            current.set(current.get() + 1);
            if current.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "test"))
            } else {
                Ok(())
            }
        };
        {
            let mut admission = Admission::with_residency(u64::MAX, budget.clone());
            assert_eq!(
                RasterLayer::from_rgba_checked(
                    257,
                    2,
                    Rgba::TRANSPARENT,
                    &input,
                    &mut admission,
                    &check
                )
                .unwrap_err()
                .code,
                ErrorCode::OperationCancelled
            );
        }
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn malformed_dimensions_and_lengths_never_allocate() {
    for (w, h, input) in [
        (0, 1, vec![]),
        (1, 0, vec![]),
        (1, 1, vec![0; 3]),
        (u32::MAX, u32::MAX, vec![]),
    ] {
        let budget = SharedBudget::new(0);
        let mut admission = Admission::with_residency(u64::MAX, budget.clone());
        assert_eq!(
            RasterLayer::from_rgba_checked(
                w,
                h,
                Rgba::TRANSPARENT,
                &input,
                &mut admission,
                &|| Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
        assert_eq!(budget.usage().peak, 0);
    }
}
