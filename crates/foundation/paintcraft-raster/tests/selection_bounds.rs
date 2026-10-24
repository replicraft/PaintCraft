use paintcraft_core::types::{Error, ErrorCode, Rect, resources::SharedBudget, text::MaskSpan};
use paintcraft_raster::selection::{BinaryMask, BoundsCache, MaskLimits};
use std::cell::Cell;

fn expected(mask: &BinaryMask, clip: Rect) -> Option<Rect> {
    let mut samples = Vec::new();
    for y in clip.y0..clip.y1 {
        for x in clip.x0..clip.x1 {
            if mask.coverage(x, y) == 255 {
                samples.push((x, y));
            }
        }
    }
    samples.first()?;
    Some(Rect {
        x0: samples.iter().map(|p| p.0).min().unwrap(),
        y0: samples.iter().map(|p| p.1).min().unwrap(),
        x1: samples.iter().map(|p| p.0).max().unwrap() + 1,
        y1: samples.iter().map(|p| p.1).max().unwrap() + 1,
    })
}

#[test]
fn every_small_mask_view_clip_and_inversion_matches_independent_selected_samples() {
    let budget = SharedBudget::new(8 << 20);
    for selected in 0u16..512 {
        let domain = Rect::new(-2, -1, 3, 3).unwrap();
        let spans: Vec<_> = (0..9)
            .filter(|index| selected >> index & 1 != 0)
            .map(|index| MaskSpan {
                y: index / 3 - 1,
                x0: index % 3 - 2,
                x1: index % 3 - 2,
                coverage: 255,
            })
            .collect();
        let original = BinaryMask::from_spans(
            domain,
            false,
            &spans,
            &budget,
            MaskLimits::default(),
            &|| Ok(()),
        )
        .unwrap();
        for inverse in [false, true] {
            let mask = if inverse {
                original.inverted()
            } else {
                original.clone()
            };
            for delta in [(0, 0), (-255, 257)] {
                let mask = mask.translated(delta.0, delta.1).unwrap();
                for (x, y, w, h) in [
                    (-4, -3, 7, 7),
                    (-2, -1, 3, 3),
                    (-2, 0, 1, 1),
                    (-1, -2, 3, 3),
                    (1, 2, 2, 2),
                ] {
                    let clip = Rect::new(x + delta.0, y + delta.1, w, h).unwrap();
                    assert_eq!(
                        mask.bounds_checked(clip, 100_000, &|| Ok(())).unwrap(),
                        expected(&mask, clip)
                    );
                    if let Some(clipped) = mask.clipped(clip).unwrap() {
                        assert_eq!(
                            clipped.bounds_checked(clip, 100_000, &|| Ok(())).unwrap(),
                            expected(&clipped, clip)
                        );
                    }
                }
            }
        }
    }
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn huge_uniform_sparse_and_sparse_inverted_masks_do_not_scan_implicit_pixels() {
    let domain = Rect::new(-500_000, -500_000, 1_048_576, 1_048_576).unwrap();
    for selected in [false, true] {
        let mask = BinaryMask::uniform(domain, selected).unwrap();
        assert_eq!(
            mask.bounds_checked(domain, 1, &|| Ok(())).unwrap(),
            selected.then_some(domain)
        );
        assert!(mask.bounds_checked(domain, 0, &|| Ok(())).is_err());
    }
    let budget = SharedBudget::new(1 << 20);
    let mask = BinaryMask::from_spans(
        domain,
        false,
        &[MaskSpan {
            y: 400_000,
            x0: 350_000,
            x1: 350_003,
            coverage: 255,
        }],
        &budget,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        mask.bounds_checked(domain, 70_000, &|| Ok(())).unwrap(),
        Some(Rect::new(350_000, 400_000, 4, 1).unwrap())
    );
    assert_eq!(
        mask.inverted()
            .bounds_checked(domain, 100, &|| Ok(()))
            .unwrap(),
        Some(domain)
    );
    assert_eq!(
        mask.bounds_checked(Rect::new(-500_000, -500_000, 100, 100).unwrap(), 2, &|| Ok(
            ()
        ))
        .unwrap(),
        None
    );
}

#[test]
fn empty_tile_band_skips_only_proven_zero_rows_and_dense_edge_rows_are_exact() {
    let domain = Rect::new(-256, -256, 512, 768).unwrap();
    let budget = SharedBudget::new(4 << 20);
    let spans: Vec<_> = (-256..0)
        .map(|y| MaskSpan {
            y,
            x0: -256,
            x1: 255,
            coverage: 0,
        })
        .collect();
    let mask = BinaryMask::from_spans(
        domain,
        true,
        &spans,
        &budget,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        mask.bounds_checked(domain, 100, &|| Ok(())).unwrap(),
        Some(Rect::new(-256, 0, 512, 512).unwrap())
    );
    // Dense edge exceptions: a zero first row must not skip selected row 1.
    let partial = BinaryMask::from_spans(
        Rect::new(0, 0, 4, 4).unwrap(),
        true,
        &[
            MaskSpan {
                y: 0,
                x0: 0,
                x1: 3,
                coverage: 0,
            },
            MaskSpan {
                y: 3,
                x0: 0,
                x1: 3,
                coverage: 0,
            },
        ],
        &budget,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        partial
            .bounds_checked(partial.domain(), 1000, &|| Ok(()))
            .unwrap(),
        Some(Rect::new(0, 1, 4, 2).unwrap())
    );
}

#[test]
fn shared_work_and_each_cancellation_checkpoint_return_no_partial_result_or_storage_change() {
    let budget = SharedBudget::new(1 << 20);
    let mask = BinaryMask::from_spans(
        Rect::new(-10, -10, 200, 200).unwrap(),
        false,
        &[MaskSpan {
            y: 5,
            x0: -2,
            x1: 17,
            coverage: 255,
        }],
        &budget,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let bytes = budget.usage().reserved;
    let calls = Cell::new(0);
    let expected = mask
        .bounds_checked(mask.domain(), 1 << 20, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    for failure in 1..=calls.get() {
        let seen = Cell::new(0);
        let error = mask
            .bounds_checked(mask.domain(), 1 << 20, &|| {
                seen.set(seen.get() + 1);
                if seen.get() == failure {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel bounds"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(budget.usage().reserved, bytes);
    }
    let mut work = 1 << 20;
    assert_eq!(
        mask.bounds_with_remaining_checked(mask.domain(), &mut work, &|| Ok(()))
            .unwrap(),
        expected
    );
    assert!(work < 1 << 20);
    work = 1;
    assert_eq!(
        mask.bounds_with_remaining_checked(mask.domain(), &mut work, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(work, 0);
    let mut cache = BoundsCache::default();
    assert_eq!(cache.get(&mask, mask.domain()).unwrap(), expected);
    assert_eq!(cache.get(&mask, mask.domain()).unwrap(), expected);
    assert_eq!(
        cache.get(&mask.inverted(), mask.domain()).unwrap(),
        Some(mask.domain())
    );
}

#[test]
fn bounds_cache_clear_releases_its_only_retained_view_without_geometry_allocations() {
    let budget = SharedBudget::new(1 << 20);
    let domain = Rect::new(0, 0, 8, 8).unwrap();
    let mask = BinaryMask::from_spans(
        domain,
        false,
        &[MaskSpan {
            y: 3,
            x0: 2,
            x1: 4,
            coverage: 255,
        }],
        &budget,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let before = budget.usage().reserved;
    let mut cache = BoundsCache::default();
    assert_eq!(
        cache.get(&mask, domain).unwrap(),
        Some(Rect::new(2, 3, 3, 1).unwrap())
    );
    assert_eq!(budget.usage().reserved, before);
    drop(mask);
    assert_eq!(
        budget.usage().reserved,
        before,
        "immutable cache key owns its mask view"
    );
    cache.clear();
    assert_eq!(budget.usage().reserved, 0);
}
