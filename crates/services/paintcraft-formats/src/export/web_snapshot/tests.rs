use super::*;
use paintcraft_core::types::Rgba;
const DDS: Limits = crate::dds::web_export::SOURCE_LIMITS;
const MSP: Limits = crate::msp::web_export::SOURCE_LIMITS;
fn doc(w: u32, h: u32) -> Document {
    paintcraft_document::new_document(
        w,
        h,
        Some(Rgba([0, 0, 0, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap()
}
fn raster(w: u32, h: u32) -> paintcraft_raster::RasterLayer {
    let pixels = (0..h)
        .flat_map(|y| {
            (0..w).flat_map(move |x| {
                let c = if (x + y) % 2 == 0 { 0 } else { 255 };
                [c, c, c, 255]
            })
        })
        .collect::<Vec<_>>();
    paintcraft_raster::RasterLayer::from_rgba(
        w,
        h,
        Rgba([0, 0, 0, 255]),
        &pixels,
        &mut paintcraft_raster::Admission::new(128 << 20),
    )
    .unwrap()
}
#[test]
fn source_side_and_tile_caps_are_distinct_without_truncating_skinny_sources() {
    let dds = DDS;
    let msp = MSP;
    assert!(Plan::capture(&doc(65536, 1), dds).is_ok());
    assert!(Plan::capture(&doc(65536, 1), msp).is_err());
    let mut source = doc(65535, 1);
    source.layers.get_mut(&source.roots[0]).unwrap().raster = raster(65535, 1);
    assert_eq!(source.layers[&source.roots[0]].raster.resident_tiles(), 256);
    assert!(Plan::capture(&source, dds).is_err());
    let plan = Plan::capture(&source, msp).unwrap();
    let bytes = plan.snapshot(&source, |_| Ok(())).unwrap();
    assert!(
        bytes.len() < 1 << 20,
        "raw PCND writes logical edge samples, not padded tiles"
    );
    let reopened = crate::pcnd::read(&bytes).unwrap();
    assert_eq!(reopened.canvas, source.canvas);
    assert_eq!(reopened.layers[&reopened.roots[0]].raster.width(), 65535);
}
#[test]
fn full_layer_samples_need_the_larger_raw_limit_and_remain_byte_identical() {
    let mut source = doc(2048, 2048);
    source.layers.get_mut(&source.roots[0]).unwrap().raster = raster(2048, 2048);
    let mut layer = source.layers[&source.roots[0]].clone();
    layer.id = Uuid::new_v4();
    source.roots.push(layer.id);
    source.layers.insert(layer.id, layer);
    let profile = MSP;
    let plan = Plan::capture(&source, profile).unwrap();
    let bytes = plan.snapshot(&source, |_| Ok(())).unwrap();
    assert!(bytes.len() > 32 << 20);
    assert_eq!(bytes, crate::pcnd::write(&source).unwrap());
    let tight = Plan::capture(
        &source,
        Limits {
            snapshot: 32 << 20,
            ..profile
        },
    )
    .unwrap();
    assert_eq!(
        tight.snapshot(&source, |_| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}
#[test]
fn stale_identity_and_codec_layout_checks_precede_snapshot_allocation() {
    let mut source = doc(7, 5);
    let plan = Plan::capture(&source, Limits { snapshot: 1, ..MSP }).unwrap();
    source.state_id = Uuid::new_v4();
    assert_eq!(
        plan.snapshot(&source, |_| Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    let plan = Plan::capture(&source, Limits { snapshot: 1, ..MSP }).unwrap();
    let error = plan
        .snapshot(&source, |_| {
            Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "selected layout",
            ))
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
}
