//! Tight limits distinguish a priced1px request from an unpriced32² cache tile.
use super::*;
use paintcraft_render::{
    cache::{CacheLimits, CompositeCache, Domain},
    display::{DisplayCache, DisplayRequest},
};
fn limits() -> CacheLimits {
    CacheLimits {
        tile_edge: 32,
        committed_bytes: 1 << 20,
        preview_bytes: 1 << 20,
        entries_per_domain: 16,
    }
}
fn data() -> Value {
    json::parse(include_bytes!(
        "../layer-blend-cpu-fixtures/bounds-oracle.json"
    ))
    .unwrap()
}
fn from_row(row: &Value) -> Document {
    let modes = row.get("modes").unwrap().as_array().unwrap();
    let back = Rgba(rgba(row.get("backdrop").unwrap()).try_into().unwrap());
    let mut d = scene(
        back,
        Rgba([204, 77, 13, 128]),
        if modes[0].as_str() == Some("multiply") {
            Mode::Multiply
        } else {
            Mode::Screen
        },
        64,
    );
    d.canvas = (64, 64);
    let ids = d.roots.clone();
    d.layers.get_mut(&ids[0]).unwrap().raster = RasterLayer::new(64, 64, back);
    let front = d.layers.get_mut(&ids[1]).unwrap();
    front.raster = RasterLayer::new(64, 64, Rgba([204, 77, 13, 128]));
    front.opacity = 64;
    if modes.len() == 2 {
        let mut next = front.clone();
        next.id = deterministic_id(&["bounds-second"]);
        next.blend_mode = Mode::Screen;
        next.raster = RasterLayer::new(64, 64, Rgba([40, 190, 230, 128]));
        d.roots.push(next.id);
        d.layers.insert(next.id, next);
    }
    d
}
fn dirty(d: &mut Document) {
    let id = d.roots[1];
    d.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .blend_pixel(
            0,
            0,
            Rgba([128, 64, 200, 128]),
            paintcraft_raster::BlendMode::Replace,
            &mut paintcraft_raster::Admission::new(65536),
        )
        .unwrap();
}
// Keep a real tile present before warming the cache. Creating its first tile is
// a structural change, for which the cache correctly recomposes instead of patches.
fn patchable_from_row(row: &Value, side: u32) -> Document {
    let mut d = from_row(row);
    d.canvas = (side, side);
    for layer in d.layers.values_mut() {
        layer.raster = RasterLayer::new(side, side, layer.raster.default_pixel());
    }
    let id = d.roots[1];
    let original = d.layers[&id].raster.default_pixel();
    dirty(&mut d);
    d.layers
        .get_mut(&id)
        .unwrap()
        .raster
        .blend_pixel(
            0,
            0,
            original,
            paintcraft_raster::BlendMode::Replace,
            &mut paintcraft_raster::Admission::new(65536),
        )
        .unwrap();
    assert_eq!(d.layers[&id].raster.resident_tiles(), 1);
    d
}
#[test]
fn cold_dirty_and_display_zero_lod_keep_single_pixel_work_for_one_and_two_modes() {
    let oracle = data();
    let roi = Rect::new(0, 0, 1, 1).unwrap();
    for row in oracle.get("cases").unwrap().as_array().unwrap() {
        let limit = row.get("limit").unwrap().as_i64().unwrap() as u64;
        let mut d = patchable_from_row(row, 64);
        let mut cache = CompositeCache::new(limits()).unwrap();
        let a = workspace(limit);
        let first = with_workspace(a.clone(), || {
            cache.render(&d, roi, Domain::Committed, || Ok(()))
        })
        .unwrap();
        assert_eq!(first.pixels, rgba(row.get("initial").unwrap()));
        assert_eq!(first.stats.recomposed_pixels, 1);
        assert_eq!(a.work_used(), limit);
        dirty(&mut d);
        let a = workspace(limit);
        let changed = with_workspace(a.clone(), || {
            cache.render(&d, roi, Domain::Committed, || Ok(()))
        })
        .unwrap();
        assert_eq!(changed.pixels, rgba(row.get("dirty").unwrap()));
        // A one-pixel request has no unchanged pixels to preserve.
        assert_eq!(changed.stats.patched_tiles, 0);
        assert_eq!(changed.stats.recomposed_pixels, 1);
        assert_eq!(a.work_used(), limit);
        let mut display = DisplayCache::new(limits()).unwrap();
        let mut d = patchable_from_row(row, 64);
        let request = DisplayRequest::new(&d, roi, 0).unwrap();
        let a = workspace(limit);
        let first = with_workspace(a.clone(), || {
            display.render(&d, request, Domain::Committed, || Ok(()))
        })
        .unwrap();
        assert_eq!(first.pixels, rgba(row.get("initial").unwrap()));
        assert_eq!(first.stats.native_pixels, 1);
        assert_eq!(a.work_used(), limit);
        dirty(&mut d);
        let a = workspace(limit);
        let changed = with_workspace(a.clone(), || {
            display.render(&d, request, Domain::Committed, || Ok(()))
        })
        .unwrap();
        assert_eq!(changed.pixels, rgba(row.get("dirty").unwrap()));
        assert_eq!(changed.stats.native_pixels, 1);
        assert_eq!(a.work_used(), limit);
    }
}
#[test]
fn stored_blend_outside_roi_clips_cache_and_display_without_charging_extra_pixels() {
    let oracle = data();
    let row = &oracle.get("cases").unwrap().as_array().unwrap()[0];
    let mut d = from_row(row);
    let front = d.layers.get_mut(&d.roots[1]).unwrap();
    front.origin = (16, 0);
    front.raster = RasterLayer::new(48, 64, Rgba([204, 77, 13, 128]));
    let roi = Rect::new(0, 0, 1, 1).unwrap();
    let mut cache = CompositeCache::new(limits()).unwrap();
    let a = workspace(512);
    let frame = with_workspace(a.clone(), || {
        cache.render(&d, roi, Domain::Committed, || Ok(()))
    })
    .unwrap();
    assert_eq!(frame.pixels, [255; 4]);
    assert_eq!(frame.stats.recomposed_pixels, 1);
    assert_eq!(a.work_used(), 512);
    let request = DisplayRequest::new(&d, roi, 0).unwrap();
    let mut display = DisplayCache::new(limits()).unwrap();
    let a = workspace(512);
    let frame = with_workspace(a.clone(), || {
        display.render(&d, request, Domain::Committed, || Ok(()))
    })
    .unwrap();
    assert_eq!(frame.pixels, [255; 4]);
    assert_eq!(frame.stats.native_pixels, 1);
    assert_eq!(a.work_used(), 512);
}

#[test]
fn partial_dirty_patches_clip_native_cells_to_the_requested_one_row() {
    let oracle = data();
    let roi = Rect::new(0, 0, 65, 1).unwrap();
    for row in oracle.get("cases").unwrap().as_array().unwrap() {
        let modes = row.get("modes").unwrap().as_array().unwrap().len() as u64;
        let setup = if modes == 1 { 512 } else { 768 };
        let per_pixel = if modes == 1 { 32 } else { 64 };
        let limit = setup + 65 * per_pixel;
        let initial = rgba(row.get("initial").unwrap()).repeat(65);
        let mut expected = initial.clone();
        expected[..4].copy_from_slice(&rgba(row.get("dirty").unwrap()));
        let mut d = patchable_from_row(row, 128);
        let mut cache_limits = limits();
        cache_limits.tile_edge = 128;
        let mut cache = CompositeCache::new(cache_limits).unwrap();
        let first = with_workspace(workspace(limit), || {
            cache.render(&d, roi, Domain::Committed, || Ok(()))
        })
        .unwrap();
        assert_eq!(first.pixels, initial);
        dirty(&mut d);
        let a = workspace(limit);
        let changed = with_workspace(a.clone(), || {
            cache.render(&d, roi, Domain::Committed, || Ok(()))
        })
        .unwrap();
        assert_eq!(changed.pixels, expected);
        assert_eq!(changed.stats.patched_tiles, 1);
        assert_eq!(changed.stats.recomposed_pixels, 64);
        assert_eq!(a.work_used(), setup + 64 * per_pixel);
        let mut d = patchable_from_row(row, 128);
        let request = DisplayRequest::new(&d, roi, 0).unwrap();
        // Display tiles are capped at 64 pixels; its second tile stays cached.
        cache_limits.tile_edge = 64;
        let mut display = DisplayCache::new(cache_limits).unwrap();
        let first = with_workspace(workspace(limit), || {
            display.render(&d, request, Domain::Committed, || Ok(()))
        })
        .unwrap();
        assert_eq!(first.pixels, initial);
        dirty(&mut d);
        let a = workspace(limit);
        let changed = with_workspace(a.clone(), || {
            display.render(&d, request, Domain::Committed, || Ok(()))
        })
        .unwrap();
        assert_eq!(changed.pixels, expected);
        assert_eq!(changed.stats.native_pixels, 64);
        assert_eq!(a.work_used(), setup + 64 * per_pixel);
    }
}
