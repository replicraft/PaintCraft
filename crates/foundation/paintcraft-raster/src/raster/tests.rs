//! Sparse raster storage and admission regressions.
use super::*;

const RED: Rgba = Rgba([255, 0, 0, 255]);

fn adm() -> Admission {
    Admission::new(1 << 24)
}

#[test]
fn euclidean_tile_addressing_handles_negatives_and_edges() {
    assert_eq!(tile_split(0), (0, 0));
    assert_eq!(tile_split(255), (0, 255));
    assert_eq!(tile_split(256), (1, 0));
    assert_eq!(tile_split(-1), (-1, 255));
    assert_eq!(tile_split(-256), (-1, 0));
    assert_eq!(tile_split(-257), (-2, 255));
}

#[test]
fn two_by_two_layer_stores_hidden_rgb() {
    let mut l = RasterLayer::new(2, 2, Rgba([0, 0, 255, 255]));
    let mut a = adm();
    l.blend_pixel(0, 0, Rgba([255, 0, 0, 0]), BlendMode::Replace, &mut a)
        .unwrap();
    assert_eq!(l.get(0, 0), Rgba([255, 0, 0, 0]));
    assert_eq!(l.get(1, 1), Rgba([0, 0, 255, 255]));
    let mut samples = Vec::new();
    l.for_each_row(|r| samples.extend_from_slice(r));
    assert_eq!(
        samples,
        [255, 0, 0, 0, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255]
    );
}

#[test]
fn read_span_crosses_tiles_and_matches_get() {
    let mut a = adm();
    let mut l = RasterLayer::new(600, 2, Rgba([1, 1, 1, 1]));
    l.fill_rect(
        &Rect::new(250, 1, 12, 1).unwrap(),
        RED,
        BlendMode::Replace,
        &mut a,
    )
    .unwrap();
    let mut buf = vec![0u8; 20 * 4];
    l.read_span(1, 245, &mut buf);
    for (i, px) in buf.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(Rgba(*px), l.get(245 + i as i64, 1), "x = {}", 245 + i);
    }
    let mut whole = Vec::new();
    l.for_each_row(|r| whole.extend_from_slice(r));
    assert_eq!(whole.len(), 600 * 2 * 4);
}

#[test]
fn writes_across_tile_boundaries_and_out_of_bounds_are_clipped() {
    let mut l = RasterLayer::new(300, 3, Rgba::TRANSPARENT);
    let mut a = adm();
    l.fill_rect(
        &Rect::new(254, 0, 4, 1).unwrap(),
        RED,
        BlendMode::Replace,
        &mut a,
    )
    .unwrap();
    l.blend_pixel(300, 0, RED, BlendMode::Replace, &mut a)
        .unwrap();
    l.blend_pixel(-1, 0, RED, BlendMode::Replace, &mut a)
        .unwrap();
    for x in 254..258 {
        assert_eq!(l.get(x, 0), RED);
    }
    assert_eq!(
        (l.get(253, 0), l.get(258, 0), l.get(299, 0)),
        (Rgba::TRANSPARENT, Rgba::TRANSPARENT, Rgba::TRANSPARENT)
    );
    assert_eq!(l.resident_tiles(), 2);
    let blocks = l.blocks();
    assert_eq!(
        blocks
            .iter()
            .map(|b| (b.0, b.1, b.2, b.3))
            .collect::<Vec<_>>(),
        [(0, 0, 256, 3), (1, 0, 44, 3)]
    );
}

#[test]
fn default_valued_writes_do_not_allocate_and_equal_empty_layers() {
    let mut l = RasterLayer::new(512, 512, Rgba::TRANSPARENT);
    let mut a = adm();
    l.fill_rect(
        &l.bounds(),
        Rgba([9, 9, 9, 0]),
        BlendMode::SourceOver,
        &mut a,
    )
    .unwrap();
    assert_eq!(l.resident_tiles(), 0);
    l.blend_pixel(10, 10, RED, BlendMode::Replace, &mut a)
        .unwrap();
    l.blend_pixel(10, 10, Rgba::TRANSPARENT, BlendMode::Replace, &mut a)
        .unwrap();
    assert_eq!(l, RasterLayer::new(512, 512, Rgba::TRANSPARENT));
    assert!(l.blocks().is_empty());
    // Hidden RGB is not the default.
    l.blend_pixel(10, 10, Rgba([5, 0, 0, 0]), BlendMode::Replace, &mut a)
        .unwrap();
    assert_ne!(l, RasterLayer::new(512, 512, Rgba::TRANSPARENT));
    assert_eq!(l.blocks().len(), 1);
}

#[test]
fn clones_share_tiles_until_written() {
    let mut a = adm();
    let mut original = RasterLayer::new(10, 10, Rgba::TRANSPARENT);
    original
        .blend_pixel(1, 1, RED, BlendMode::Replace, &mut a)
        .unwrap();
    let before = a.remaining_pixels;
    let mut copy = original.clone();
    assert_eq!(copy, original);
    copy.blend_pixel(2, 2, RED, BlendMode::Replace, &mut a)
        .unwrap();
    assert_eq!(
        before - a.remaining_pixels,
        TILE_PIXELS as u64,
        "copy-on-write is charged"
    );
    assert_eq!(original.get(2, 2), Rgba::TRANSPARENT);
    assert_ne!(copy, original);
}

#[test]
fn exhausting_the_budget_is_a_resource_error() {
    let mut l = RasterLayer::new(1024, 1024, Rgba::TRANSPARENT);
    let mut a = Admission::new((TILE_PIXELS * 2) as u64);
    let e = l
        .fill_rect(&l.bounds(), RED, BlendMode::Replace, &mut a)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::ResourceLimit);
}

#[test]
fn from_rgba_matches_pixel_by_pixel_construction_and_skips_default_tiles() {
    let (w, h) = (300u32, 270u32);
    let mut px = vec![0u8; (w * h * 4) as usize];
    for (x, y) in [(0usize, 0usize), (299, 269), (256, 3), (3, 256)] {
        px[(y * w as usize + x) * 4..][..4].copy_from_slice(&[x as u8, y as u8, 7, 255]);
    }
    px[(10 * w as usize + 10) * 4..][..3].copy_from_slice(&[9, 9, 9]); // hidden RGB, alpha 0
    let mut a = adm();
    let l = RasterLayer::from_rgba(w, h, Rgba::TRANSPARENT, &px, &mut a).unwrap();
    let mut rows = Vec::new();
    l.for_each_row(|r| rows.extend_from_slice(r));
    assert_eq!(rows, px);
    assert_eq!(l.resident_tiles(), 4);
    let blank =
        RasterLayer::from_rgba(w, h, Rgba::TRANSPARENT, &vec![0; px.len()], &mut a).unwrap();
    assert_eq!(blank.resident_tiles(), 0);
    assert!(RasterLayer::from_rgba(w, h, Rgba::TRANSPARENT, &px[1..], &mut a).is_err());
}

#[test]
fn blocks_round_trip_through_set_block() {
    let mut a = adm();
    let mut l = RasterLayer::new(300, 260, Rgba([1, 2, 3, 4]));
    l.fill_rect(
        &Rect::new(255, 255, 2, 2).unwrap(),
        RED,
        BlendMode::Replace,
        &mut a,
    )
    .unwrap();
    let mut m = RasterLayer::new(300, 260, Rgba([1, 2, 3, 4]));
    for (bx, by, w, h, data) in l.blocks() {
        assert_eq!(data.len() as u32, w * h * 4);
        m.set_block(bx, by, &data, &mut a).unwrap();
    }
    assert_eq!(l, m);
    assert!(m.set_block(2, 0, &[0; 4], &mut a).is_err());
    assert!(m.set_block(0, 0, &[0; 4], &mut a).is_err());
}
#[test]
fn expansion_preserves_hidden_rgb_and_shared_tiles_across_partial_tile_edges() {
    let default = Rgba([12, 34, 56, 0]);
    let mut layer = RasterLayer::new(259, 257, default);
    let mut admission = Admission::new(65536);
    layer
        .blend_pixel(
            258,
            256,
            Rgba([9, 8, 7, 255]),
            BlendMode::Replace,
            &mut admission,
        )
        .unwrap();
    let original = layer.clone();
    let identities: Vec<_> = layer.tile_identities().collect();
    layer.expand_to(300, 300).unwrap();
    assert_eq!(layer.tile_identities().collect::<Vec<_>>(), identities);
    assert_eq!(layer.get(258, 256), Rgba([9, 8, 7, 255]));
    for (x, y) in [(259, 256), (258, 257), (299, 299)] {
        assert_eq!(layer.get(x, y), default);
    }
    assert_eq!((original.width(), original.height()), (259, 257));
    assert!(layer.expand_to(1, 300).is_err());
    assert_eq!((layer.width(), layer.height()), (300, 300));
}
