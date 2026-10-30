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
        "compact PCND writes logical edge samples, not padded tiles"
    );
    let reopened = crate::pcnd::read(&bytes).unwrap();
    assert_eq!(reopened.canvas, source.canvas);
    assert_eq!(reopened.layers[&reopened.roots[0]].raster.width(), 65535);
}
#[test]
fn full_layer_samples_use_selected_compact_without_relaxing_snapshot_caps() {
    let mut source = doc(2048, 2048);
    source.layers.get_mut(&source.roots[0]).unwrap().raster = raster(2048, 2048);
    let mut layer = source.layers[&source.roots[0]].clone();
    layer.id = Uuid::new_v4();
    source.roots.push(layer.id);
    source.layers.insert(layer.id, layer);
    let profile = MSP;
    let plan = Plan::capture(&source, profile).unwrap();
    // Keep the independently meaningful full logical source: RAW exceeds32MiB
    // because both2048² layers and all canonical headers remain present.
    let raw = crate::pcnd::write(&source).unwrap();
    assert!(raw.len() > 32 << 20);
    assert!(raw.len() <= profile.snapshot);
    let bytes = plan.snapshot(&source, |_| Ok(())).unwrap();
    assert_eq!(bytes, crate::pcnd::write_compact(&source).unwrap());
    assert!(crate::pcnd::read(&bytes).unwrap().same_semantics(&source));
    // A smaller wire allowance may now admit compression, not fewer samples.
    let stored = Plan::capture(
        &source,
        Limits {
            snapshot: 32 << 20,
            ..profile
        },
    )
    .unwrap();
    assert_eq!(stored.snapshot(&source, |_| Ok(())).unwrap(), bytes);
    // The same exact stored-size boundary still refuses before publication.
    let tight = Plan::capture(
        &source,
        Limits {
            snapshot: bytes.len() - 1,
            ..profile
        },
    )
    .unwrap();
    assert_eq!(
        tight.snapshot(&source, |_| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        crate::pcnd::write(&source).unwrap(),
        raw,
        "failed snapshot never changes source"
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

#[test]
fn full_filter_capacity_has_separate_storage_quote_and_same_id_digest_freshness() {
    use super::filter_bake::{self, Intent};
    use paintcraft_core::types::filters::Descriptor;
    use paintcraft_document::{FilterInstance, FilterStack, filters::FILTERS_MAX};
    let mut source = doc(1, 1);
    let first = source.layers[&source.roots[0]].clone();
    for number in 1..8 {
        let mut layer = first.clone();
        layer.id = Uuid::from_u128(number + 100);
        source.roots.push(layer.id);
        source.layers.insert(layer.id, layer);
    }
    for layer in source.layers.values_mut() {
        layer.filters = FilterStack::new(
            (0..FILTERS_MAX)
                .map(|number| {
                    FilterInstance::new(
                        Uuid::from_u128(number as u128),
                        false,
                        Descriptor::InvertRgbV1,
                    )
                })
                .collect(),
        )
        .unwrap();
    }
    let target = Target::new(
        crate::registry::Family::Msp,
        crate::registry::Operation::ExportFlat,
    );
    let bake = filter_bake::Policy::capture(&source, target, Intent::Bake, &|| Ok(())).unwrap();
    let plan = Plan::capture_with_policies(&source, MSP, Policy::default(), bake, target).unwrap();
    let inventory = plan.filter_inventory();
    assert_eq!(inventory.filter_count(), (8 * FILTERS_MAX) as u64);
    assert_eq!(
        inventory.filter_storage_bytes(),
        filter_storage_ceiling(8).unwrap()
    );
    assert_eq!(plan.flags(), 5); // Flatten and disabled editable intentions.
    let bytes = plan.snapshot(&source, |_| Ok(())).unwrap();
    let reopened = crate::pcnd::read(&bytes).unwrap();
    assert_eq!(
        filter_bake::inspect(&reopened, &|| Ok(()))
            .unwrap()
            .digest(),
        inventory.digest()
    );
    let quote = pre_read_quote(bytes.len(), MSP).unwrap();
    assert!(
        quote
            >= bytes.len() as u64
                + MSP.resident
                + (8 << 20)
                + (1 << 20)
                + inventory.filter_storage_bytes()
    );
    assert_eq!(
        pre_read_quote(
            bytes.len(),
            Limits {
                resident: u64::MAX,
                ..MSP
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    // No source pixels or state UUID change; the cached semantic stack changes.
    source.layers.get_mut(&source.roots[0]).unwrap().filters = FilterStack::default();
    assert_eq!(
        plan.snapshot(&source, |_| Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
}
