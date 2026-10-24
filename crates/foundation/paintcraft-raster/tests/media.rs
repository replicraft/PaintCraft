use paintcraft_core::types::{Error, ErrorCode, Rect, brush::BrushMedia};
use paintcraft_raster::media::coverage;
use std::collections::{BTreeMap, BTreeSet};

fn pixels(media: BrushMedia, points: &[(i64, i64)], clip: Rect) -> BTreeMap<(i64, i64), u8> {
    coverage(points, 16, 1, media, clip, &|| Ok(()))
        .unwrap()
        .into_iter()
        .flat_map(|span| (span.x0..=span.x1).map(move |x| ((x, span.y), span.coverage)))
        .collect()
}
#[test]
fn all_media_are_distinct_repeatable_antialiased_and_clip_without_changing_the_stroke() {
    let clip = Rect::new(0, 0, 96, 64).unwrap();
    let inner = Rect::new(22, 13, 35, 23).unwrap();
    let mut images = BTreeSet::new();
    for media in BrushMedia::ALL.into_iter().skip(1) {
        let path = [(8, 9), (80, 48), (19, 45)];
        let full = pixels(media, &path, clip);
        assert!(!full.is_empty(), "{media:?}");
        assert!(full.values().any(|&alpha| alpha < 255), "{media:?}");
        assert_eq!(full, pixels(media, &path, clip));
        assert_eq!(
            pixels(media, &path, inner),
            full.iter()
                .filter(|((x, y), _)| inner.contains(*x, *y))
                .map(|(p, c)| (*p, *c))
                .collect(),
            "{media:?}"
        );
        assert!(images.insert(full), "{media:?} must have its own coverage");
    }
}
#[test]
fn translucent_media_union_retraced_dabs_and_nibs_have_opposite_diagonals() {
    let clip = Rect::new(0, 0, 48, 48).unwrap();
    for media in BrushMedia::ALL.into_iter().skip(1) {
        assert_eq!(
            pixels(media, &[(10, 20), (35, 20)], clip),
            pixels(media, &[(10, 20), (35, 20), (10, 20)], clip),
            "{media:?}"
        );
    }
    let a = pixels(BrushMedia::CalligraphyBrush, &[(20, 20)], clip);
    let b = pixels(BrushMedia::CalligraphyPen, &[(20, 20)], clip);
    assert!(a.contains_key(&(24, 24)) && !a.contains_key(&(24, 16)));
    assert!(b.contains_key(&(24, 16)) && !b.contains_key(&(24, 24)));
}
#[test]
fn invalid_requests_scratch_overflow_and_cancellation_fail_before_publication() {
    let clip = Rect::new(0, 0, 4096, 4096).unwrap();
    assert_eq!(
        coverage(
            &[(0, 0), (4095, 4095)],
            16,
            1,
            BrushMedia::Oil,
            clip,
            &|| Ok(())
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        coverage(&[(4, 4)], 16, 1, BrushMedia::Oil, clip, &|| Err(
            Error::new(ErrorCode::OperationCancelled, "cancel")
        ))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert!(coverage(&[], 16, 1, BrushMedia::Oil, clip, &|| Ok(())).is_err());
}

#[test]
fn artistic_strokes_deplete_and_spray_has_discrete_particles() {
    let clip = Rect::new(0, 0, 320, 64).unwrap();
    for media in [BrushMedia::Oil, BrushMedia::Watercolor] {
        let stroke = pixels(media, &[(16, 32), (300, 32)], clip);
        let early: u64 = (12..=20)
            .filter_map(|x| stroke.get(&(x, 32)))
            .map(|&a| u64::from(a))
            .sum();
        let late: u64 = (200..=208)
            .filter_map(|x| stroke.get(&(x, 32)))
            .map(|&a| u64::from(a))
            .sum();
        assert!(early > 0);
        assert_eq!(late, 0, "{media:?} should run out along a long stroke");
    }
    let spray = pixels(BrushMedia::Airbrush, &[(16, 32)], clip);
    assert!(spray.values().any(|&alpha| alpha == 255));
    assert!((12..=20).any(|x| !spray.contains_key(&(x, 32))));
}
