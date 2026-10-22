use paintcraft_core::types::{Error, ErrorCode, Rect};
use paintcraft_raster::font::{self, visit_layout};
use std::cell::Cell;

#[test]
fn streamed_text_matches_independent_glyph_pixels_and_legacy_order() {
    let text = "Aé\nZ A";
    let cells = [
        ('A', 0, 0),
        ('é', 1, 0),
        ('Z', 0, 1),
        (' ', 1, 1),
        ('A', 2, 1),
    ];
    for scale in [1, 2, 7, 16] {
        for at in [(-17, -11), (0, 0), (7, 5)] {
            for clip in [
                Rect::new(0, 0, 43, 31).unwrap(),
                Rect::new(-11, -7, 23, 17).unwrap(),
            ] {
                let mut spans = Vec::new();
                visit_layout(text, at, scale, &clip, &|| Ok(()), |s| {
                    spans.push(s);
                    Ok(())
                })
                .unwrap();
                assert_eq!(spans, font::layout(text, at, scale, &clip).unwrap());
                for y in clip.y0..clip.y1 {
                    for x in clip.x0..clip.x1 {
                        let expected = cells.iter().any(|&(c, col, line)| {
                            let gx = at.0 + col * 8 * scale;
                            let gy = at.1 + line * 8 * scale;
                            let (dx, dy) = ((x - gx).div_euclid(scale), (y - gy).div_euclid(scale));
                            (0..8).contains(&dx)
                                && (0..8).contains(&dy)
                                && font::glyph(c).unwrap()[dy as usize] >> dx & 1 != 0
                        });
                        assert_eq!(
                            spans.iter().any(|s| s.y == y && s.x0 <= x && x <= s.x1),
                            expected
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn visitor_cancels_at_each_checkpoint_and_stops_on_the_first_sink_error() {
    let clip = Rect::new(0, 0, 100, 40).unwrap();
    let calls = Cell::new(0);
    visit_layout(
        "AA\nZ",
        (0, 0),
        2,
        &clip,
        &|| {
            calls.set(calls.get() + 1);
            Ok(())
        },
        |_| Ok(()),
    )
    .unwrap();
    for cancel in 1..=calls.get() {
        let seen = Cell::new(0);
        let error = visit_layout(
            "AA\nZ",
            (0, 0),
            2,
            &clip,
            &|| {
                seen.set(seen.get() + 1);
                if seen.get() == cancel {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel text"))
                } else {
                    Ok(())
                }
            },
            |_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(seen.get(), cancel);
    }
    let mut emitted = 0;
    let error = visit_layout("AA", (0, 0), 1, &clip, &|| Ok(()), |_| {
        emitted += 1;
        Err(Error::new(ErrorCode::ResourceLimit, "sink exhausted"))
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(emitted, 1);
}

#[test]
fn clipped_characters_still_validate_and_wide_origins_do_not_wrap() {
    let clip = Rect::new(-8, -8, 16, 16).unwrap();
    for at in [(i64::MIN, i64::MIN), (i64::MAX, i64::MAX)] {
        let mut emitted = 0;
        visit_layout("AA\nA", at, 16, &clip, &|| Ok(()), |_| {
            emitted += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(emitted, 0);
        assert_eq!(
            visit_layout("A漢", at, 1, &clip, &|| Ok(()), |_| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    for scale in [i64::MIN, 0, 17, i64::MAX] {
        assert_eq!(
            visit_layout("A", (0, 0), scale, &clip, &|| Ok(()), |_| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    assert_eq!(
        visit_layout(&"A".repeat(1025), (0, 0), 1, &clip, &|| Ok(()), |_| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    let seen = Cell::new(0);
    let error = visit_layout(
        &"\n".repeat(1024),
        (0, 0),
        1,
        &clip,
        &|| {
            seen.set(seen.get() + 1);
            if seen.get() == 500 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel offscreen",
                ))
            } else {
                Ok(())
            }
        },
        |_| panic!("newlines emit no spans"),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
}
