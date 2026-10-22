use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba, resources::SharedBudget};
use paintcraft_raster::{Admission, BlendMode, PaintPlan, PaintSnapshot, RasterLayer, flood};
use std::cell::Cell;

fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn admission(metadata: &SharedBudget) -> Admission {
    Admission::with_residency(0, SharedBudget::new(0)).with_paint_metadata_budget(metadata.clone())
}
fn paint(adm: &Admission, area: Rect, color: Rgba) -> PaintPlan {
    let mut p = adm.paint_plan();
    p.rect(area, color, BlendMode::Replace).unwrap();
    p
}
fn cancelled() -> Error {
    Error::new(ErrorCode::OperationCancelled, "cancel source lineage")
}

#[test]
fn immutable_forks_keep_captured_pixels_and_expand_with_hidden_default_rgb() {
    let metadata = SharedBudget::new(1 << 20);
    let adm = admission(&metadata);
    let default = Rgba([13, 29, 47, 0]);
    let source = PaintSnapshot::new(RasterLayer::new(3, 2, default), &adm, &|| Ok(())).unwrap();
    let mut incoming = adm.paint_plan();
    // Descriptors extending beyond the old extent must not leak after expansion.
    incoming
        .rect(
            rect(1, 0, 20, 3),
            Rgba([250, 0, 1, 255]),
            BlendMode::Replace,
        )
        .unwrap();
    let shared = source
        .fork(&adm, &|| Ok(()))
        .unwrap()
        .append_checked(incoming, &|| Ok(()))
        .unwrap()
        .freeze_checked(&adm, &|| Ok(()))
        .unwrap();
    let sibling = shared.clone();
    let a = shared
        .fork_expanded(6, 4, &adm, &|| Ok(()))
        .unwrap()
        .append_checked(
            paint(&adm, rect(4, 3, 1, 1), Rgba([0, 255, 0, 255])),
            &|| Ok(()),
        )
        .unwrap();
    let b = sibling
        .fork(&adm, &|| Ok(()))
        .unwrap()
        .append_checked(paint(&adm, rect(1, 0, 1, 1), default), &|| Ok(()))
        .unwrap();
    let ar = a.reader(&adm, &|| Ok(())).unwrap();
    let br = b.reader(&adm, &|| Ok(())).unwrap();
    let sr = sibling.reader(&adm, &|| Ok(())).unwrap();
    for y in 0..4 {
        for x in 0..6 {
            let original = if x < 3 && y < 2 && x >= 1 {
                Rgba([250, 0, 1, 255])
            } else {
                default
            };
            let expected = if (x, y) == (4, 3) {
                Rgba([0, 255, 0, 255])
            } else {
                original
            };
            assert_eq!(ar.read_checked(x, y, &|| Ok(())).unwrap(), expected);
            assert_eq!(sr.read_checked(x, y, &|| Ok(())).unwrap(), original);
            let expected = if (x, y) == (1, 0) { default } else { original };
            assert_eq!(br.read_checked(x, y, &|| Ok(())).unwrap(), expected);
        }
    }
    assert!(shared.fork_expanded(2, 2, &adm, &|| Ok(())).is_err());
    assert_eq!(adm.residency_usage().peak, 0);
    drop((ar, br, sr));
    drop((a, b, sibling, shared, source));
    assert_eq!(metadata.usage().reserved, 0);
}

#[test]
fn uniform_default_proofs_skip_huge_noops_but_never_discard_later_restoration() {
    let metadata = SharedBudget::new(1 << 20);
    let adm = admission(&metadata).with_paint_metadata_limit(4096);
    let default = Rgba([31, 63, 127, 0]);
    let source = PaintSnapshot::new(
        RasterLayer::new(1_000_000, 1_000_000, default),
        &adm,
        &|| Ok(()),
    )
    .unwrap();
    let held = metadata.usage().reserved;
    let calls = Cell::new(0);
    let branch = source
        .fork(&adm, &|| Ok(()))
        .unwrap()
        .append_checked(paint(&adm, source.bounds(), default), &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    assert!(calls.get() < 10, "no enumeration of the huge tile grid");
    assert_eq!(metadata.usage().reserved, held);
    let reader = branch.reader(&adm, &|| Ok(())).unwrap();
    assert_eq!(reader.scratch_bytes(), 0);
    assert_eq!(
        reader.read_checked(500_000, 500_000, &|| Ok(())).unwrap(),
        default
    );
    drop(reader);
    let branch = branch
        .append_checked(paint(&adm, rect(0, 0, 1, 1), Rgba([255; 4])), &|| Ok(()))
        .unwrap()
        .freeze_checked(&adm, &|| Ok(()))
        .unwrap();
    let restored = branch
        .fork(&adm, &|| Ok(()))
        .unwrap()
        .append_checked(paint(&adm, rect(0, 0, 1, 1), default), &|| Ok(()))
        .unwrap();
    assert_eq!(
        restored
            .reader(&adm, &|| Ok(()))
            .unwrap()
            .read_checked(0, 0, &|| Ok(()))
            .unwrap(),
        default
    );
    assert_eq!(
        branch
            .reader(&adm, &|| Ok(()))
            .unwrap()
            .read_checked(0, 0, &|| Ok(()))
            .unwrap(),
        Rgba([255; 4])
    );
    assert_eq!(adm.residency_usage().peak, 0);
}

#[test]
fn captured_resident_tiles_survive_external_cow_and_partial_coverage_keeps_exact_order() {
    let decoded = SharedBudget::new(4 * RasterLayer::TILE_BYTES as u64);
    let mut source = RasterLayer::new(257, 2, Rgba([19, 23, 31, 0]));
    let mut pixels = Admission::with_residency(4 * 65_536, decoded.clone());
    source
        .blend_pixel(
            255,
            1,
            Rgba([1, 2, 3, 255]),
            BlendMode::Replace,
            &mut pixels,
        )
        .unwrap();
    let metadata = SharedBudget::new(1 << 20);
    let adm = admission(&metadata);
    let snapshot = PaintSnapshot::new(source.clone(), &adm, &|| Ok(())).unwrap();
    let before = decoded.usage();
    let mut p = adm.paint_plan();
    p.coverage_rect(rect(254, 0, 3, 2), Rgba([200, 100, 50, 0]), 7)
        .unwrap();
    p.rect(
        rect(255, 1, 1, 1),
        Rgba([40, 80, 160, 128]),
        BlendMode::SourceOver,
    )
    .unwrap();
    let branch = snapshot
        .fork(&adm, &|| Ok(()))
        .unwrap()
        .append_checked(p, &|| Ok(()))
        .unwrap();
    let reader = branch.reader(&adm, &|| Ok(())).unwrap();
    assert_eq!(decoded.usage(), before);
    // Independent scalar coverage and source-over equations.
    for y in 0..2 {
        for x in 0..257 {
            let mut expected = source.get(x, y);
            if x >= 254 {
                expected = Rgba(std::array::from_fn(|i| {
                    ((u32::from([200u8, 100, 50, 0][i]) * 7 + u32::from(expected.0[i]) * 9 + 8)
                        / 16) as u8
                }));
            }
            if (x, y) == (255, 1) {
                let sa = 128u64;
                let da = u64::from(expected.a());
                let a = sa * 255 + da * (255 - sa);
                expected = Rgba(std::array::from_fn(|i| {
                    if i == 3 {
                        ((a + 127) / 255) as u8
                    } else {
                        ((u64::from([40u8, 80, 160][i]) * sa * 255
                            + u64::from(expected.0[i]) * da * (255 - sa)
                            + a / 2)
                            / a) as u8
                    }
                }));
            }
            assert_eq!(reader.read_checked(x, y, &|| Ok(())).unwrap(), expected);
        }
    }
    source
        .blend_pixel(255, 1, Rgba([99; 4]), BlendMode::Replace, &mut pixels)
        .unwrap();
    assert_eq!(
        snapshot
            .reader(&adm, &|| Ok(()))
            .unwrap()
            .read_checked(255, 1, &|| Ok(()))
            .unwrap(),
        Rgba([1, 2, 3, 255])
    );
    assert_eq!(decoded.usage().reserved, 2 * RasterLayer::TILE_BYTES as u64);
}

#[test]
fn deep_branch_lineages_read_and_drop_without_recursive_stack_growth() {
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            let metadata = SharedBudget::new(16 << 20);
            let adm = admission(&metadata);
            let mut source =
                PaintSnapshot::new(RasterLayer::new(1, 1, Rgba::TRANSPARENT), &adm, &|| Ok(()))
                    .unwrap();
            for i in 0..4096 {
                source = source
                    .fork(&adm, &|| Ok(()))
                    .unwrap()
                    .append_checked(
                        paint(&adm, rect(0, 0, 1, 1), Rgba([i as u8, 7, 8, 255])),
                        &|| Ok(()),
                    )
                    .unwrap()
                    .freeze_checked(&adm, &|| Ok(()))
                    .unwrap();
            }
            let reader = source.reader(&adm, &|| Ok(())).unwrap();
            assert_eq!(
                reader.read_checked(0, 0, &|| Ok(())).unwrap(),
                Rgba([255, 7, 8, 255])
            );
            assert!(reader.scratch_bytes() >= 4097 * std::mem::size_of::<usize>() as u64);
            assert!(
                source
                    .reader(&adm.clone().with_paint_metadata_limit(8), &|| Ok(()))
                    .is_err()
            );
            drop(reader);
            let held = metadata.usage().reserved;
            let branch = source.fork_expanded(520, 1, &adm, &|| Ok(())).unwrap();
            let calls = Cell::new(0);
            let error = branch
                .append_checked(paint(&adm, rect(256, 0, 1, 1), Rgba::TRANSPARENT), &|| {
                    calls.set(calls.get() + 1);
                    if calls.get() == 17 {
                        Err(cancelled())
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
            assert_eq!(metadata.usage().reserved, held);
            drop(source);
            assert_eq!(metadata.usage().reserved, 0);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn concurrent_ancestor_release_keeps_deep_teardown_bounded_and_charges_exact() {
    use std::sync::{Arc, Barrier};
    let metadata = SharedBudget::new(16 << 20);
    let adm = admission(&metadata);
    let mut source =
        PaintSnapshot::new(RasterLayer::new(1, 1, Rgba::TRANSPARENT), &adm, &|| Ok(())).unwrap();
    let mut ancestors = Vec::new();
    for i in 0..4096 {
        ancestors.push(source.clone());
        source = source
            .fork(&adm, &|| Ok(()))
            .unwrap()
            .append_checked(
                paint(&adm, rect(0, 0, 1, 1), Rgba([i as u8, 7, 8, 255])),
                &|| Ok(()),
            )
            .unwrap()
            .freeze_checked(&adm, &|| Ok(()))
            .unwrap();
    }
    let start = Arc::new(Barrier::new(2));
    let other = start.clone();
    let worker = std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || {
            other.wait();
            drop(source);
        })
        .unwrap();
    start.wait();
    while let Some(ancestor) = ancestors.pop() {
        drop(ancestor);
        std::thread::yield_now();
    }
    worker.join().unwrap();
    assert_eq!(metadata.usage().reserved, 0);
}

#[test]
fn failed_metadata_admission_discards_the_consumed_branch_without_changing_the_source() {
    use paintcraft_core::types::resources::AllocationClass;
    let metadata = SharedBudget::new(1 << 20);
    let adm = admission(&metadata);
    let source =
        PaintSnapshot::new(RasterLayer::new(8, 8, Rgba::TRANSPARENT), &adm, &|| Ok(())).unwrap();
    let branch = source.fork(&adm, &|| Ok(())).unwrap();
    let incoming = paint(&adm, rect(0, 0, 1, 1), Rgba([255; 4]));
    let held = metadata.usage().reserved;
    let blocker = metadata.reserve((1 << 20) - held).unwrap();
    let error = branch.append_checked(incoming, &|| Ok(())).unwrap_err();
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        AllocationClass::PaintMetadata
    );
    assert_eq!(
        metadata.usage().reserved,
        source.metadata_bytes() as u64 + (1 << 20) - held
    );
    assert_eq!(
        source
            .reader(&adm, &|| Ok(()))
            .unwrap()
            .read_checked(0, 0, &|| Ok(()))
            .unwrap(),
        Rgba::TRANSPARENT
    );
    drop(blocker);
    drop(source);
    assert_eq!(metadata.usage().reserved, 0);
    let error = PaintSnapshot::new(
        RasterLayer::new(8, 8, Rgba::TRANSPARENT),
        &adm.with_paint_metadata_limit(1),
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(error.allocation_denial().is_none());
}

#[test]
fn known_reserved_capacity_covers_source_nodes_programs_and_reader_scratch() {
    fn prepare(adm: &Admission) -> PaintSnapshot {
        let source =
            PaintSnapshot::new(RasterLayer::new(8, 8, Rgba::TRANSPARENT), adm, &|| Ok(())).unwrap();
        source
            .fork(adm, &|| Ok(()))
            .unwrap()
            .append_checked(paint(adm, rect(0, 0, 4, 4), Rgba([255; 4])), &|| Ok(()))
            .unwrap()
            .freeze_checked(adm, &|| Ok(()))
            .unwrap()
    }
    let probe = SharedBudget::new(1 << 20);
    let adm = admission(&probe);
    let source = prepare(&adm);
    let reader = source.reader(&adm, &|| Ok(())).unwrap();
    assert_eq!(
        reader.read_checked(0, 0, &|| Ok(())).unwrap(),
        Rgba([255; 4])
    );
    drop(reader);
    drop(source);
    assert_eq!(probe.usage().reserved, 0);
    let required = probe.usage().peak;
    let metadata = SharedBudget::new(required);
    let adm = Admission::new(0)
        .with_reserved_paint_metadata(metadata.reserve_capacity(required).unwrap());
    let source = prepare(&adm);
    let reader = source.reader(&adm, &|| Ok(())).unwrap();
    assert_eq!(
        reader.read_checked(0, 0, &|| Ok(())).unwrap(),
        Rgba([255; 4])
    );
    assert!(!metadata.can_reserve(1));
    drop(reader);
    drop(adm);
    assert_eq!(metadata.usage().reserved, source.metadata_bytes() as u64);
    drop(source);
    assert_eq!(metadata.usage().reserved, 0);
}

#[test]
fn every_observed_append_reader_and_read_checkpoint_can_cancel_without_changing_ancestors() {
    for operation in 0..3 {
        let metadata = SharedBudget::new(1 << 20);
        let adm = admission(&metadata);
        let root = PaintSnapshot::new(
            RasterLayer::new(520, 4, Rgba([13, 29, 47, 0])),
            &adm,
            &|| Ok(()),
        )
        .unwrap();
        let snapshot = root
            .fork(&adm, &|| Ok(()))
            .unwrap()
            .append_checked(paint(&adm, rect(254, 0, 6, 4), Rgba([255; 4])), &|| Ok(()))
            .unwrap()
            .freeze_checked(&adm, &|| Ok(()))
            .unwrap();
        let held = metadata.usage().reserved;
        let run = |stop: usize| {
            let calls = Cell::new(0);
            let check = || {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(cancelled())
                } else {
                    Ok(())
                }
            };
            let result = match operation {
                0 => snapshot
                    .fork(&adm, &|| Ok(()))
                    .unwrap()
                    .append_checked(paint(&adm, rect(254, 0, 6, 4), Rgba([1; 4])), &check)
                    .map(|_| ()),
                1 => snapshot.reader(&adm, &check).map(|_| ()),
                _ => snapshot
                    .reader(&adm, &|| Ok(()))
                    .unwrap()
                    .read_checked(255, 1, &check)
                    .map(|_| ()),
            };
            (result, calls.get())
        };
        let (ok, total) = run(usize::MAX);
        ok.unwrap();
        for stop in 1..=total {
            assert_eq!(run(stop).0.unwrap_err().code, ErrorCode::OperationCancelled);
            assert_eq!(metadata.usage().reserved, held);
        }
        assert_eq!(
            snapshot
                .reader(&adm, &|| Ok(()))
                .unwrap()
                .read_checked(255, 1, &|| Ok(()))
                .unwrap(),
            Rgba([255; 4])
        );
    }
}

#[test]
fn cancelled_capture_frees_source_storage_before_reentrant_metadata_release() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let decoded = SharedBudget::new(RasterLayer::TILE_BYTES as u64);
    let mut raster = RasterLayer::new(1, 1, Rgba::TRANSPARENT);
    raster
        .blend_pixel(
            0,
            0,
            Rgba([255; 4]),
            BlendMode::Replace,
            &mut Admission::with_residency(65_536, decoded.clone()),
        )
        .unwrap();
    let metadata = SharedBudget::new(1 << 20);
    let adm = admission(&metadata);
    let observed = Arc::new(AtomicBool::new(false));
    let recorded = observed.clone();
    let probe = metadata.clone();
    let watch = metadata
        .watch(1, move || {
            if probe.usage().reserved == 0 {
                recorded.store(decoded.usage().reserved == 0, Ordering::Release)
            }
        })
        .unwrap();
    watch.wait_for_release(true);
    watch.observe();
    let calls = Cell::new(0);
    let error = PaintSnapshot::new(raster, &adm, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 2 {
            Err(cancelled())
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert!(observed.load(Ordering::Acquire));
    assert_eq!(metadata.usage().reserved, 0);
}

#[test]
fn source_bound_flood_matches_independent_binary_grid_bfs() {
    use std::collections::{BTreeSet, VecDeque};
    let metadata = SharedBudget::new(1 << 20);
    let adm = admission(&metadata);
    for bits in 0..512 {
        let source =
            PaintSnapshot::new(RasterLayer::new(3, 3, Rgba::TRANSPARENT), &adm, &|| Ok(()))
                .unwrap();
        let mut branch = source.fork(&adm, &|| Ok(())).unwrap();
        for i in 0..9 {
            if bits & (1 << i) != 0 {
                branch = branch
                    .append_checked(
                        paint(&adm, rect(i % 3, i / 3, 1, 1), Rgba([255; 4])),
                        &|| Ok(()),
                    )
                    .unwrap();
            }
        }
        let reader = branch.reader(&adm, &|| Ok(())).unwrap();
        for seed in 0..9 {
            for tolerance in [0, 63, 254, 255] {
                let spans = flood::region_from_source_checked(
                    |x, y| reader.read_checked(x, y, &|| Ok(())),
                    (seed % 3, seed / 3),
                    &rect(0, 0, 3, 3),
                    tolerance,
                    1 << 20,
                    &|| Ok(()),
                )
                .unwrap();
                let actual: BTreeSet<_> = spans
                    .iter()
                    .flat_map(|s| (s.x0..=s.x1).map(move |x| (x, s.y)))
                    .collect();
                let mut expected = BTreeSet::new();
                let mut queue = VecDeque::from([(seed % 3, seed / 3)]);
                while let Some((x, y)) = queue.pop_front() {
                    if !(0..3).contains(&x) || !(0..3).contains(&y) || expected.contains(&(x, y)) {
                        continue;
                    }
                    if tolerance < 255 && ((bits >> (y * 3 + x)) & 1) != ((bits >> seed) & 1) {
                        continue;
                    }
                    expected.insert((x, y));
                    queue.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
                }
                assert_eq!(
                    actual, expected,
                    "bits={bits} seed={seed} tolerance={tolerance}"
                );
            }
        }
    }
    assert_eq!(metadata.usage().reserved, 0);
}
