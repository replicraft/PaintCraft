use super::*;
use std::cell::Cell;
fn image(w: u32, h: u32, read: impl Fn(u32, u32) -> Rgba) -> Image {
    Image::from_fn(w, h, &SharedBudget::new(64 << 20), &|| Ok(()), |x, y| {
        Ok(read(x, y))
    })
    .unwrap()
}
fn source(default: Rgba) -> RasterLayer {
    let mut layer = RasterLayer::new(263, 259, default);
    layer
        .blend_pixel(
            255,
            256,
            Rgba([99, 88, 77, 0]),
            BlendMode::Replace,
            &mut Admission::new(1 << 22),
        )
        .unwrap();
    layer
        .blend_pixel(
            256,
            255,
            Rgba([22, 44, 66, 127]),
            BlendMode::Replace,
            &mut Admission::new(1 << 22),
        )
        .unwrap();
    layer
}
#[test]
fn replace_matches_independent_finite_union_oracle_at_tile_edges_and_negative_origins() {
    for default in [Rgba::TRANSPARENT, Rgba([255; 4]), Rgba([11, 22, 33, 0])] {
        let source = source(default);
        let pixels = image(5, 3, |x, y| {
            Rgba([
                x as u8 + 1,
                y as u8 + 3,
                19,
                if x % 2 == 0 { 0 } else { 127 },
            ])
        });
        for origin in [
            (254, 255),
            (-2, 2),
            (2, -2),
            (-2, -2),
            (264, 260),
            (1000, 0),
        ] {
            for key in [None, Some(Rgba([1, 3, 19, 255]))] {
                let admission = Admission::new(1 << 24);
                let plan = RgbaPastePlan::prepare(
                    &source,
                    pixels.clone(),
                    origin,
                    BlendMode::Replace,
                    key,
                    &admission,
                    &|| Ok(()),
                )
                .unwrap();
                let mut replay_admission = admission;
                let out = plan.replay(&mut replay_admission, &|| Ok(())).unwrap();
                let (ox, oy) = plan.origin_offset();
                for y in 0..out.height() {
                    for x in 0..out.width() {
                        let (sx, sy) = (i64::from(x) + ox, i64::from(y) + oy);
                        let mut expected = if source.bounds().contains(sx, sy) {
                            source.get(sx, sy)
                        } else {
                            Rgba::TRANSPARENT
                        };
                        let (px, py) = (sx - origin.0, sy - origin.1);
                        if (0..5).contains(&px) && (0..3).contains(&py) {
                            let at = ((py * 5 + px) * 4) as usize;
                            let incoming = Rgba(pixels.rgba()[at..at + 4].try_into().unwrap());
                            if key.is_none_or(|k| k.0[..3] != incoming.0[..3]) {
                                expected = incoming;
                            }
                        }
                        assert_eq!(
                            out.get(i64::from(x), i64::from(y)),
                            expected,
                            "{origin:?} {default:?} {x},{y}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn source_over_preserves_destination_for_zero_alpha_and_blends_once() {
    let source = RasterLayer::new(3, 1, Rgba([0, 0, 255, 255]));
    let pixels = image(3, 1, |x, _| match x {
        0 => Rgba([255, 0, 0, 0]),
        1 => Rgba([255, 0, 0, 127]),
        _ => Rgba([255, 0, 0, 255]),
    });
    let mut adm = Admission::new(65536);
    let plan = RgbaPastePlan::prepare(
        &source,
        pixels,
        (0, 0),
        BlendMode::SourceOver,
        None,
        &adm,
        &|| Ok(()),
    )
    .unwrap();
    let out = plan.replay(&mut adm, &|| Ok(())).unwrap();
    assert_eq!(out.get(0, 0), Rgba([0, 0, 255, 255]));
    assert_eq!(out.get(1, 0), Rgba([127, 0, 128, 255]));
    assert_eq!(out.get(2, 0), Rgba([255, 0, 0, 255]));
    assert_eq!(source.get(1, 0), Rgba([0, 0, 255, 255]));
}
#[test]
fn unchanged_tiles_stay_shared_and_identical_paste_allocates_nothing() {
    let source = source(Rgba::TRANSPARENT);
    let pixels = image(1, 1, |_, _| Rgba([99, 88, 77, 0]));
    let mut adm = Admission::new(0);
    let plan = RgbaPastePlan::prepare(
        &source,
        pixels,
        (255, 256),
        BlendMode::Replace,
        None,
        &adm,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(plan.required_tiles(), 0);
    let out = plan.replay(&mut adm, &|| Ok(())).unwrap();
    let mut before = source.tile_identities().collect::<Vec<_>>();
    before.sort();
    let mut after = out.tile_identities().collect::<Vec<_>>();
    after.sort();
    assert_eq!(before, after);
}
#[test]
fn expanding_partial_tiles_clears_padding_that_was_outside_the_finite_source() {
    let mut source = RasterLayer::new(1, 1, Rgba::TRANSPARENT);
    let mut tile = Tile::filled(Rgba([17; 4]), &mut Admission::new(65536)).unwrap();
    tile.mark_all();
    source
        .tiles
        .insert(TileCoord { tx: 0, ty: 0 }, Arc::new(tile));
    let mut adm = Admission::new(65536);
    let plan = RgbaPastePlan::prepare(
        &source,
        image(1, 1, |_, _| Rgba([3; 4])),
        (3, 3),
        BlendMode::Replace,
        None,
        &adm,
        &|| Ok(()),
    )
    .unwrap();
    let out = plan.replay(&mut adm, &|| Ok(())).unwrap();
    assert_eq!(out.get(0, 0), Rgba([17; 4]));
    assert_eq!(out.get(2, 2), Rgba::TRANSPARENT);
    assert_eq!(out.get(3, 3), Rgba([3; 4]));
}
#[test]
fn complete_admission_and_cancellation_never_allocate_partial_results() {
    let source = RasterLayer::new(260, 1, Rgba::TRANSPARENT);
    let pixels = image(260, 1, |_, _| Rgba([3; 4]));
    let plan = RgbaPastePlan::prepare(
        &source,
        pixels,
        (0, 0),
        BlendMode::Replace,
        None,
        &Admission::new(1 << 20),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(plan.required_tiles(), 2);
    let budget = SharedBudget::new(RasterLayer::TILE_BYTES as u64);
    let mut adm = Admission::with_residency(1 << 20, budget.clone());
    assert_eq!(
        plan.replay(&mut adm, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, 0);
    let calls = Cell::new(0);
    plan.replay(&mut Admission::new(1 << 20), &|| {
        calls.set(calls.get() + 1);
        Ok(())
    })
    .unwrap();
    for fail in 1..=calls.get() {
        let count = Cell::new(0);
        let budget = SharedBudget::new(2 * RasterLayer::TILE_BYTES as u64);
        let mut adm = Admission::with_residency(1 << 20, budget.clone());
        let error = plan
            .replay(&mut adm, &|| {
                count.set(count.get() + 1);
                if count.get() == fail {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        drop(adm);
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn plan_freezes_the_old_source_and_metadata_failures_release_their_accounts() {
    let mut source = source(Rgba::TRANSPARENT);
    let pixels = image(1, 1, |_, _| Rgba([3; 4]));
    let mut adm = Admission::new(1 << 20);
    let plan = RgbaPastePlan::prepare(
        &source,
        pixels.clone(),
        (-1, -1),
        BlendMode::Replace,
        None,
        &adm,
        &|| Ok(()),
    )
    .unwrap();
    let before = source.get(255, 256);
    source
        .blend_pixel(255, 256, Rgba([5; 4]), BlendMode::Replace, &mut adm)
        .unwrap();
    let out = plan.replay(&mut adm, &|| Ok(())).unwrap();
    assert_eq!(out.get(256, 257), before);
    let budget = SharedBudget::new(0);
    let adm = Admission::new(1 << 20).with_paint_metadata_budget(budget.clone());
    assert_eq!(
        RgbaPastePlan::prepare(
            &source,
            pixels.clone(),
            (0, 0),
            BlendMode::Replace,
            None,
            &adm,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, 0);
    let budget = SharedBudget::new(1 << 20);
    let adm = Admission::new(1 << 20).with_paint_metadata_budget(budget.clone());
    let source = RasterLayer::new(1, 1, Rgba([255; 4]));
    assert_eq!(
        RgbaPastePlan::prepare(
            &source,
            pixels,
            (100000, 100000),
            BlendMode::Replace,
            None,
            &adm,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, 0);
}
