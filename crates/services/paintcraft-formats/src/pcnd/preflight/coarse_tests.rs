use super::*;
use paintcraft_core::types::Rgba;
use paintcraft_document::POLICY;
use paintcraft_raster::{TileCoord, TileLease};
use std::cell::Cell;

fn populated(tiles: usize) -> Document {
    let mut doc =
        paintcraft_document::new_document(1, 1, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    // 4096 valid full-width tile locations, each sharing the same admitted,
    // immutable payload. This tests actual resident entries without 1GiB copies.
    doc.canvas = (1_048_576, TILE_EDGE);
    let mut raster = RasterLayer::new(doc.canvas.0, doc.canvas.1, Rgba::TRANSPARENT);
    let tile = TileLease::materialize(Rgba([7, 19, 31, 255]), &mut Admission::new(65536), |_| {
        Ok(())
    })
    .unwrap();
    for x in 0..tiles {
        raster
            .install_tile(
                TileCoord {
                    tx: x as i64,
                    ty: 0,
                },
                tile.clone(),
            )
            .unwrap();
    }
    doc.layers.values_mut().next().unwrap().raster = raster;
    doc
}

#[test]
fn ordinary_acceptance_callback_count_is_independent_of_resident_tile_count() {
    for tiles in [1, 4096] {
        let doc = populated(tiles);
        assert_eq!(
            doc.layers.values().next().unwrap().raster.resident_tiles(),
            tiles
        );
        let callbacks = Cell::new(0);
        let check = || {
            callbacks.set(callbacks.get() + 1);
            Ok(())
        };
        let coarse = coarse_bound(&doc, &check).unwrap().unwrap();
        assert_eq!(callbacks.get(), 2);
        callbacks.set(0);
        let precise = upper_bound(&doc, &check).unwrap().unwrap();
        assert_eq!(callbacks.get(), tiles + 2);
        assert!(coarse >= precise && coarse < DOCUMENT_MAX);
        for compact in [false, true] {
            callbacks.set(0);
            if compact {
                check_compact(&doc, &check)
            } else {
                super::check(&doc, &check)
            }
            .unwrap();
            assert_eq!(callbacks.get(), 3);
        }
    }
}

#[test]
fn unproven_shapes_fall_back_without_turning_the_bound_into_admission() {
    for kind in 0..4 {
        let mut doc = populated(1);
        match kind {
            0 => doc.roots.push(doc.roots[0]),
            1 => doc.layers.values_mut().next().unwrap().origin.0 = i64::MAX,
            2 => doc.canvas.0 = 0,
            _ => doc.layers.values_mut().next().unwrap().name = "x".repeat(16385),
        }
        assert!(coarse_bound(&doc, &|| Ok(())).unwrap().is_none());
        assert!(upper_bound(&doc, &|| Ok(())).unwrap().is_none());
        for compact in [false, true] {
            let expected = if compact {
                binary::preflight_compact_metadata(&doc, &|| Ok(()))
            } else {
                binary::preflight_metadata(&doc, &|| Ok(()))
            }
            .unwrap_err();
            let actual = if compact {
                check_compact(&doc, &|| Ok(()))
            } else {
                super::check(&doc, &|| Ok(()))
            }
            .unwrap_err();
            assert_eq!(actual.code, expected.code);
        }
    }
}

#[test]
fn every_coarse_fast_acceptance_checkpoint_observes_one_shot_and_sticky_cancellation() {
    let doc = populated(4096);
    for compact in [false, true] {
        for sticky in [false, true] {
            for stop in 1..=3 {
                let count = Cell::new(0);
                let check = || {
                    count.set(count.get() + 1);
                    if count.get() == stop || sticky && count.get() > stop {
                        Err(Error::new(ErrorCode::OperationCancelled, "coarse cancel"))
                    } else {
                        Ok(())
                    }
                };
                let error = if compact {
                    check_compact(&doc, &check)
                } else {
                    super::check(&doc, &check)
                }
                .unwrap_err();
                assert_eq!(error.code, ErrorCode::OperationCancelled);
                assert_eq!(count.get(), stop);
            }
        }
    }
}
