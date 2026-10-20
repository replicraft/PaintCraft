use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_raster::{Admission, BlendMode};
use paintcraft_render::{
    parallel::{Partition, render_in_pool},
    render_region,
};
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixture() -> Document {
    let mut doc = new_document(259, 257, Some(Rgba([17, 45, 82, 137])), 42, POLICY).unwrap();
    let template = doc.layers[&doc.roots[0]].clone();
    let mut admission = Admission::new(16 * 65536);
    for n in 0..4 {
        let mut layer = template.clone();
        layer.id = paintcraft_core::types::ids::deterministic_id(&["parallel", &n.to_string()]);
        layer.origin = (n * 5 - 9, 7 - n * 3);
        layer.opacity = [0, 73, 128, 255][n as usize];
        layer.visible = n != 2;
        for (x, y) in [(0, 0), (258, 256), (123, 37)] {
            layer
                .raster
                .blend_pixel(
                    x,
                    y,
                    Rgba([255, 3, 37, (n * 61) as u8]),
                    BlendMode::Replace,
                    &mut admission,
                )
                .unwrap();
        }
        doc.roots.push(layer.id);
        doc.layers.insert(layer.id, layer);
    }
    doc
}

#[test]
fn disjoint_bands_and_tile_views_match_exact_ordered_regions_at_one_two_and_four_workers() {
    let doc = fixture();
    let before: Vec<_> = doc
        .layers_in_order()
        .flat_map(|l| l.raster.tile_identities())
        .collect();
    for threads in [1, 2, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        for region in [doc.canvas_rect(), Rect::new(13, 19, 239, 231).unwrap()] {
            let expected = render_region(&doc, region).unwrap();
            for partition in [
                Partition::Bands { rows: 1 },
                Partition::Bands { rows: 64 },
                Partition::Bands { rows: 256 },
                Partition::Tiles { edge: 3 },
                Partition::Tiles { edge: 64 },
                Partition::Tiles { edge: 256 },
            ] {
                assert_eq!(
                    render_in_pool(&pool, &doc, region, partition, 8 << 20, || Ok(())).unwrap(),
                    expected,
                    "threads={threads} {partition:?}"
                );
            }
        }
    }
    assert_eq!(
        before,
        doc.layers_in_order()
            .flat_map(|l| l.raster.tile_identities())
            .collect::<Vec<_>>()
    );
}

#[test]
fn work_stays_on_the_supplied_pool_and_reentrant_calls_do_not_create_threads() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .thread_name(|n| format!("qualified-render-{n}"))
        .build()
        .unwrap();
    let doc = fixture();
    let expected = render_region(&doc, doc.canvas_rect()).unwrap();
    // Reentrant install must reuse this bounded pool rather than a global pool.
    pool.install(|| {
        let pixels = render_in_pool(
            &pool,
            &doc,
            doc.canvas_rect(),
            Partition::Tiles { edge: 64 },
            1 << 20,
            || {
                assert!(
                    std::thread::current()
                        .name()
                        .unwrap()
                        .starts_with("qualified-render-")
                );
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(pixels, expected);
    });
}

#[test]
fn cancellation_bad_parameters_and_view_admission_never_return_partial_pixels() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let doc = fixture();
    for partition in [Partition::Bands { rows: 64 }, Partition::Tiles { edge: 64 }] {
        let checks = AtomicUsize::new(0);
        assert_eq!(
            render_in_pool(&pool, &doc, doc.canvas_rect(), partition, 1 << 20, || {
                if checks.fetch_add(1, Ordering::Relaxed) > 400 {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "injected cancellation",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
        assert_eq!(
            render_in_pool(
                &pool,
                &doc,
                doc.canvas_rect(),
                partition,
                1 << 20,
                || Ok(())
            )
            .unwrap(),
            render_region(&doc, doc.canvas_rect()).unwrap()
        );
    }
    for partition in [
        Partition::Bands { rows: 0 },
        Partition::Tiles { edge: 1025 },
    ] {
        assert_eq!(
            render_in_pool(&pool, &doc, doc.canvas_rect(), partition, 0, || Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    assert_eq!(
        render_in_pool(
            &pool,
            &doc,
            doc.canvas_rect(),
            Partition::Tiles { edge: 1 },
            1,
            || Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        render_in_pool(
            &pool,
            &doc,
            Rect::new(-1, 0, 1, 1).unwrap(),
            Partition::Bands { rows: 1 },
            0,
            || Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
    let huge = new_document(
        1_048_576,
        1_048_576,
        Some(Rgba([12, 34, 56, 255])),
        0,
        POLICY,
    )
    .unwrap();
    for partition in [
        Partition::Bands { rows: 1024 },
        Partition::Tiles { edge: 1024 },
    ] {
        assert_eq!(
            render_in_pool(
                &pool,
                &huge,
                Rect::new(900_000, 850_000, 3, 2).unwrap(),
                partition,
                1024,
                || Ok(())
            )
            .unwrap(),
            [12, 34, 56, 255].repeat(6)
        );
    }
}
