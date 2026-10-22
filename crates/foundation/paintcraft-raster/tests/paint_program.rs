use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba, resources::SharedBudget};
use paintcraft_raster::{Admission, BlendMode, PaintPlan, PaintProjection, RasterLayer, flood};
use std::{
    cell::Cell,
    collections::{BTreeSet, VecDeque},
};
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn plan(limit: usize, area: Rect, color: Rgba, mode: BlendMode) -> PaintPlan {
    let mut p = PaintPlan::with_metadata_limit(limit);
    p.rect(area, color, mode).unwrap();
    p
}
fn initial(limit: usize, bounds: Rect) -> PaintProjection {
    PaintPlan::with_metadata_limit(limit)
        .into_projection_checked(bounds, &|| Ok(()))
        .unwrap()
}
fn cancel() -> Error {
    Error::new(ErrorCode::OperationCancelled, "cancel combined paint")
}

#[test]
fn combined_writes_preserve_exact_order_and_read_the_source_only_once() {
    let bounds = rect(0, 0, 520, 4);
    let background = Rgba([31, 63, 127, 0]);
    let mut program = initial(1 << 20, bounds);
    let mut expected = vec![background; 520 * 4];
    for i in 0..64 {
        let area = rect((i % 8) * 40 - 4, i % 4, 280, 2);
        let color = Rgba([i as u8, 88, 156, 128]);
        let mut incoming = PaintPlan::default();
        if i % 3 == 0 {
            incoming.coverage_rect(area, color, 7).unwrap();
        } else {
            incoming.rect(area, color, BlendMode::SourceOver).unwrap();
        }
        program = program.append_checked(incoming, &|| Ok(())).unwrap();
        for y in 0..4 {
            for x in 0..520 {
                if area.contains(x, y) {
                    let dst = &mut expected[(y * 520 + x) as usize];
                    *dst = if i % 3 == 0 {
                        Rgba(std::array::from_fn(|j| {
                            ((u32::from(color.0[j]) * 7 + u32::from(dst.0[j]) * 9 + 8) / 16) as u8
                        }))
                    } else {
                        color.source_over(*dst)
                    };
                }
            }
        }
        assert!(program.metadata_bytes() <= 1 << 20);
    }
    for y in 0..4 {
        for x in 0..520 {
            let reads = Cell::new(0);
            let actual = program
                .read_checked(
                    x,
                    y,
                    |_, _| {
                        reads.set(reads.get() + 1);
                        Ok(background)
                    },
                    &|| Ok(()),
                )
                .unwrap();
            assert_eq!(actual, expected[(y * 520 + x) as usize]);
            assert_eq!(reads.get(), 1);
        }
    }
    let mut replay = RasterLayer::new(520, 4, background);
    program
        .into_plan()
        .apply(&mut replay, &mut Admission::new(3 * 65_536))
        .unwrap();
    for y in 0..4 {
        for x in 0..520 {
            assert_eq!(replay.get(x, y), expected[(y * 520 + x) as usize]);
        }
    }
}

#[test]
fn aggregate_cap_covers_accumulated_indices_and_incoming_transfer_capacity() {
    let bounds = rect(0, 0, 8, 8);
    let mut program = initial(512, bounds);
    let mut appended = 0;
    loop {
        let incoming = plan(
            512,
            rect(0, 0, 1, 1),
            Rgba([1, 2, 3, 128]),
            BlendMode::SourceOver,
        );
        match program.append_checked(incoming, &|| Ok(())) {
            Ok(next) => {
                program = next;
                appended += 1;
                assert!(program.metadata_bytes() <= 512);
                assert!(appended < 64);
            }
            Err(error) => {
                assert_eq!(error.code, ErrorCode::ResourceLimit);
                assert!(error.allocation_denial().is_none());
                break;
            }
        }
    }
    assert!(appended > 1);
    let program = initial(512, bounds);
    let mut incoming = PaintPlan::default();
    for _ in 0..64 {
        incoming
            .rect(rect(100, 100, 1, 1), Rgba([1; 4]), BlendMode::Replace)
            .unwrap();
    }
    assert!(incoming.descriptor_bytes() > 512);
    let calls = Cell::new(0);
    let error = program
        .append_checked(incoming, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(error.allocation_denial().is_none());
    assert!(
        calls.get() <= 2,
        "transfer denial must precede descriptor/index traversal"
    );
}

#[test]
fn every_observed_append_checkpoint_cancels_the_consumed_private_program() {
    let bounds = rect(0, 0, 520, 4);
    let build = || {
        let program = plan(1 << 20, rect(0, 0, 1, 1), Rgba([1; 4]), BlendMode::Replace)
            .into_projection_checked(bounds, &|| Ok(()))
            .unwrap();
        let mut incoming = PaintPlan::default();
        for y in 0..4 {
            incoming
                .rect(
                    rect(0, y, 520, 1),
                    Rgba([17, 88, 156, 128]),
                    BlendMode::SourceOver,
                )
                .unwrap();
        }
        (program, incoming)
    };
    let total = Cell::new(0);
    let (program, incoming) = build();
    program
        .append_checked(incoming, &|| {
            total.set(total.get() + 1);
            Ok(())
        })
        .unwrap();
    for at in 1..=total.get() {
        let calls = Cell::new(0);
        let (program, incoming) = build();
        let error = program
            .append_checked(incoming, &|| {
                calls.set(calls.get() + 1);
                if calls.get() == at {
                    Err(cancel())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(calls.get(), at);
    }
}

#[test]
fn combined_operation_replay_admits_all_tiles_before_allocating_a_prefix() {
    let bounds = rect(0, 0, 300, 1);
    let program = initial(4096, bounds)
        .append_checked(
            plan(4096, rect(0, 0, 1, 1), Rgba([1; 4]), BlendMode::Replace),
            &|| Ok(()),
        )
        .unwrap()
        .append_checked(
            plan(4096, rect(299, 0, 1, 1), Rgba([2; 4]), BlendMode::Replace),
            &|| Ok(()),
        )
        .unwrap();
    let bytes = RasterLayer::TILE_BYTES as u64;
    let budget = SharedBudget::new(bytes);
    let mut destination = RasterLayer::new(300, 1, Rgba([31, 63, 127, 0]));
    let error = program
        .into_plan()
        .apply(
            &mut destination,
            &mut Admission::with_residency(2 * 65_536, budget.clone()),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(
        error.allocation_denial().unwrap().required_bytes(),
        2 * bytes
    );
    assert_eq!(destination.resident_tiles(), 0);
    assert_eq!(budget.usage().peak, 0);
}

fn bfs(pixels: &[Rgba; 9], seed: usize, tolerance: u8) -> BTreeSet<(i64, i64)> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([seed]);
    while let Some(i) = queue.pop_front() {
        let (x, y) = ((i % 3) as i64, (i / 3) as i64);
        if seen.contains(&(x, y))
            || pixels[i]
                .0
                .iter()
                .zip(pixels[seed].0)
                .any(|(&a, b)| a.abs_diff(b) > tolerance)
        {
            continue;
        }
        seen.insert((x, y));
        for (x, y) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if (0..3).contains(&x) && (0..3).contains(&y) {
                queue.push_back((y * 3 + x) as usize);
            }
        }
    }
    seen
}
#[test]
fn combined_flood_matches_independent_bfs_over_all_small_binary_sources() {
    let bounds = rect(0, 0, 3, 3);
    let wall = Rgba([1, 2, 3, 255]);
    let erase = Rgba([9, 8, 7, 0]);
    let program = initial(4096, bounds)
        .append_checked(
            plan(4096, rect(1, 0, 1, 2), wall, BlendMode::Replace),
            &|| Ok(()),
        )
        .unwrap()
        .append_checked(
            plan(4096, rect(1, 1, 2, 1), erase, BlendMode::Replace),
            &|| Ok(()),
        )
        .unwrap();
    for bits in 0..512 {
        let base: [Rgba; 9] = std::array::from_fn(|i| {
            if bits >> i & 1 == 0 {
                Rgba([5, 10, 15, 0])
            } else {
                Rgba([200, 80, 20, 255])
            }
        });
        let mut expected = base;
        expected[1] = wall;
        expected[4] = erase;
        expected[5] = erase;
        for seed in 0..9 {
            for tolerance in [0, 16, 128, 255] {
                let spans = flood::region_from_source_checked(
                    |x, y| {
                        program.read_checked(
                            x,
                            y,
                            |x, y| Ok(base[(y * 3 + x) as usize]),
                            &|| Ok(()),
                        )
                    },
                    ((seed % 3) as i64, (seed / 3) as i64),
                    &bounds,
                    tolerance,
                    64 << 10,
                    &|| Ok(()),
                )
                .unwrap();
                let actual: BTreeSet<_> = spans
                    .iter()
                    .flat_map(|s| (s.x0..=s.x1).map(move |x| (x, s.y)))
                    .collect();
                assert_eq!(actual, bfs(&expected, seed, tolerance));
            }
        }
    }
}
