use super::*;
use crate::raster::portable_pixels::decode;
use crate::raster::portable_pixels::{Meter, PixelGeometry};
#[test]
fn full_owned_decode_is_adopted_without_second_pixel_allocation() {
    let q = SharedBudget::new(2 * 262144);
    let reservation = q.reserve_capacity(262144).unwrap();
    let controls = SharedBudget::new(4096).scope(AllocationClass::PaintMetadata);
    let factories = ReservedDecodeFactories {
        selection_budget: SharedBudget::new(4096),
        coverage_budget: SharedBudget::new(4096),
        paint_metadata_scope: controls.clone(),
    };
    let mut admission = Admission::with_reserved_decode_factories(
        65536,
        reservation.clone(),
        factories,
        &controls,
        &|| Ok(()),
    )
    .unwrap();
    let b = SharedBudget::new(2 << 20);
    let sc = b
        .reserve_capacity(4096 + 262144)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let mut c = sc.account();
    c.resize(4096).unwrap();
    let mut m = b.account(AllocationClass::PaintMetadata);
    m.resize(if cfg!(target_pointer_width = "64") {
        356
    } else {
        308
    })
    .unwrap();
    let check = || Ok(());
    let spend = |_| Ok(());
    let mut meter = Meter {
        check: &check,
        spend: &spend,
        charged: 0,
        tile_header_bytes: 0,
    };
    let g = PixelGeometry {
        bx: 0,
        by: 0,
        w: 256,
        h: 256,
        decoded: 262144,
        encoded: 6,
        runs_extent: 0,
        coord: TileCoord { tx: 0, ty: 0 },
    };
    let p = EncodedTilePayload::new(PortablePixelEncoding::Runs, &[255, 255, 7, 8, 9, 0]);
    let d = decode::expand(p, g, &sc, &mut meter).unwrap().unwrap();
    let pointer = d.bytes.as_ptr();
    let pending = admission
        .portable_pending((Some(d), p), g, Rgba::TRANSPARENT, &mut meter)
        .unwrap();
    assert_eq!(pending.pixels.as_ptr(), pointer);
    assert_eq!(reservation.available(), 0);
    assert_eq!(sc.footprint().live, 4096 + 262144);
    drop(pending);
    assert_eq!(reservation.available(), 262144);
    assert_eq!(sc.footprint().live, 4096);
    drop(c);
    assert_eq!(sc.footprint().live, 0);
    drop(m);
}
