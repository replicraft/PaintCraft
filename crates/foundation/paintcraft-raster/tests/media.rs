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
fn loaded_paint_remains_continuous_and_only_feathers_at_exposed_edges() {
    let clip = Rect::new(0, 0, 640, 64).unwrap();
    for media in [BrushMedia::Watercolor] {
        let stroke = pixels(media, &[(16, 32), (620, 32)], clip);
        for x in 16..=620 {
            assert!(
                stroke.get(&(x, 32)).copied().unwrap_or(0) >= 75,
                "{media:?} x={x}"
            );
        }
        let middle = stroke[&(300, 32)];
        let edge = stroke.get(&(300, 24)).copied().unwrap_or(0);
        assert!(middle > edge, "{media:?} must taper at the exposed edge");
        // A return along the same line neither reloads nor darkens its pigment.
        assert_eq!(
            stroke,
            pixels(media, &[(16, 32), (620, 32), (16, 32)], clip)
        );
    }
}

#[test]
fn marker_is_dark_and_crayon_leaves_clustered_paper_breaks() {
    let clip = Rect::new(0, 0, 320, 64).unwrap();
    let path = [(16, 32), (300, 32)];
    let marker = pixels(BrushMedia::Marker, &path, clip);
    assert!((24..290).all(|x| marker[&(x, 32)] >= 245));
    let crayon = pixels(BrushMedia::Crayon, &path, clip);
    let pencil = pixels(BrushMedia::NaturalPencil, &path, clip);
    let area: Vec<_> = (24..290)
        .flat_map(|x| (29..36).map(move |y| (x, y)))
        .collect();
    let missing = |map: &BTreeMap<_, _>| area.iter().filter(|p| !map.contains_key(*p)).count();
    let mean = |map: &BTreeMap<_, u8>| {
        area.iter()
            .map(|p| u64::from(*map.get(p).unwrap_or(&0)))
            .sum::<u64>()
            / area.len() as u64
    };
    let gaps = missing(&crayon);
    assert!(
        (area.len() / 8..area.len() / 2).contains(&gaps),
        "wax gaps {gaps}/{}",
        area.len()
    );
    assert!(mean(&crayon) > mean(&pencil) + 20);
    assert!(
        (24..289).any(|x| !crayon.contains_key(&(x, 32)) && !crayon.contains_key(&(x + 1, 32))),
        "wax flakes span adjacent pixels"
    );
    let spray = pixels(BrushMedia::Airbrush, &[(16, 32)], clip);
    assert!(spray.values().any(|&alpha| alpha == 255));
    assert!((12..=20).any(|x| !spray.contains_key(&(x, 32))));
}

#[test]
fn preview_prefix_grows_without_repainting_previous_coverage() {
    let clip = Rect::new(0, 0, 160, 80).unwrap();
    for medium in BrushMedia::ALL.into_iter().skip(1) {
        let prefix = pixels(medium, &[(12, 30), (70, 30)], clip);
        let full = pixels(medium, &[(12, 30), (70, 30), (140, 60)], clip);
        assert!(
            prefix
                .iter()
                .all(|(p, alpha)| full.get(p).is_some_and(|next| next >= alpha)),
            "{medium:?}"
        );
        // Points behind the new segment's footprint remain exactly unchanged.
        for (&p, &alpha) in prefix.iter().filter(|((x, _), _)| *x < 60) {
            assert_eq!(full[&p], alpha, "{medium:?} at {p:?}");
        }
    }
}

#[test]
fn angled_nibs_do_not_leave_periodic_pits_along_straight_edges() {
    let clip = Rect::new(0, 0, 160, 64).unwrap();
    for medium in [BrushMedia::CalligraphyBrush, BrushMedia::CalligraphyPen] {
        let stroke = pixels(medium, &[(10, 30), (140, 30)], clip);
        for y in 20..40 {
            let row: Vec<_> = (30..120)
                .map(|x| stroke.get(&(x, y)).copied().unwrap_or(0))
                .collect();
            let variation = row.iter().max().unwrap() - row.iter().min().unwrap();
            assert!(variation <= 16, "{medium:?} y={y} variation={variation}");
        }
    }
}

#[test]
fn oil_load_fades_and_narrows_along_distance_without_repainting_earlier_marks() {
    let clip = Rect::new(0, 0, 640, 64).unwrap();
    let stroke = pixels(BrushMedia::Oil, &[(16, 32), (620, 32)], clip);
    assert!(stroke[&(40, 32)] > 240);
    assert!(stroke[&(180, 32)] < stroke[&(80, 32)]);
    assert!(stroke.get(&(400, 32)).copied().unwrap_or(0) < 10);
    assert!(!stroke.contains_key(&(600, 32)));
    let width = |x| stroke.keys().filter(|(column, _)| *column == x).count();
    assert!(width(40) > width(240));
    assert_eq!(
        stroke,
        pixels(BrushMedia::Oil, &[(16, 32), (620, 32), (16, 32)], clip)
    );
}
