//! Real offscreen GPU outputs use independent literals, never CPU rendering.
use super::*;
use paintcraft_core::types::{
    Rect,
    filters::{BlackWhiteV1, Descriptor, PosterizeRgbV1, ThresholdLumaV1},
    resources::SharedBudget,
};
use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use paintcraft_render::display::DisplayRequest;

const RAW: [u8; 4] = [20, 115, 200, 255];
struct Case {
    descriptor: Descriptor,
    expected: [u8; 4],
    changed: Descriptor,
    changed_expected: [u8; 4],
}
fn cases() -> [Case; 4] {
    [
        Case {
            descriptor: Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(100).unwrap()),
            expected: [61, 108, 150, 255],
            changed: Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(0).unwrap()),
            changed_expected: RAW,
        },
        Case {
            descriptor: Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(128).unwrap()),
            expected: [10, 57, 100, 255],
            changed: Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(0).unwrap()),
            changed_expected: [138, 185, 228, 255],
        },
        Case {
            descriptor: Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(6).unwrap()),
            expected: [10, 108, 202, 255],
            changed: Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(2).unwrap()),
            changed_expected: [10, 57, 228, 255],
        },
        Case {
            descriptor: Descriptor::SolarizeRgbV1,
            expected: [20, 115, 127, 255],
            changed: Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(3).unwrap()),
            changed_expected: [10, 122, 228, 255],
        },
    ]
}
fn stack(descriptor: Descriptor, mask: Option<FilterMask>) -> FilterStack {
    FilterStack::new(vec![
        FilterInstance::new(deterministic_id(&["gpu-point-profile"]), true, descriptor)
            .with_mask(mask),
    ])
    .unwrap()
}
fn renderer(adapter: &wgpu::Adapter, single: bool) -> GpuRenderer {
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    GpuRenderer::new(device, queue)
        .unwrap()
        .with_single_layer_display(single)
}
#[test]
fn four_literal_masked_profiles_reach_gpu_general_single_and_descriptor_cache_keys() {
    let Some(adapter) = adapter() else { return };
    for single in [false, true] {
        let gpu = renderer(&adapter, single);
        let mut doc = new_document(2, 1, Some(Rgba(RAW)), 91, POLICY).unwrap();
        let id = doc.roots[0];
        let identity = (doc.document_id, doc.state_id);
        let raster = doc.layers[&id].raster.clone();
        let versions: Vec<_> = raster.tile_versions(doc.canvas_rect()).unwrap().collect();
        assert!(versions.is_empty());
        let coverage = A8Mask::uniform(
            Rect::new(0, 0, 2, 1).unwrap(),
            128,
            &SharedBudget::new(4096),
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap();
        let mask_digest = coverage.digest();
        let storage = coverage.storage_usage();
        for case in cases() {
            doc.layers.get_mut(&id).unwrap().filters = stack(
                case.descriptor,
                Some(FilterMask::new(true, coverage.clone())),
            );
            let expected = case.expected.repeat(2);
            let first = gpu.render_with_stats(&doc, || Ok(())).unwrap();
            assert_eq!(
                first.pixels, expected,
                "single={single} {:?}",
                case.descriptor
            );
            let warm = gpu.render_with_stats(&doc, || Ok(())).unwrap();
            assert_eq!(warm.pixels, expected);
            assert!(warm.stats.source_hits > 0);
            let request = DisplayRequest::new(&doc, doc.canvas_rect(), 0).unwrap();
            let old_display = gpu
                .render_display_texture(&doc, request, || Ok(()))
                .unwrap();
            assert_eq!(old_display.stats.single_layer_displays, u64::from(single));
            assert_eq!(old_display.readback(|| Ok(())).unwrap(), expected);
            let old = doc.layers[&id].filters.as_slice()[0].clone();
            doc.layers.get_mut(&id).unwrap().filters =
                FilterStack::new(vec![old.with_descriptor(case.changed)]).unwrap();
            let changed = gpu.render_with_stats(&doc, || Ok(())).unwrap();
            assert_eq!(changed.pixels, case.changed_expected.repeat(2));
            assert_ne!(changed.pixels, expected);
            assert!(
                changed.stats.source_allocations > 0,
                "descriptor-only edit must miss an older appearance"
            );
            let changed_display = gpu
                .render_display_texture(&doc, request, || Ok(()))
                .unwrap();
            assert_eq!(
                changed_display.stats.single_layer_displays,
                u64::from(single)
            );
            assert_eq!(
                changed_display.readback(|| Ok(())).unwrap(),
                case.changed_expected.repeat(2)
            );
            assert_eq!(old_display.readback(|| Ok(())).unwrap(), expected);
            let mask = doc.layers[&id].filters.as_slice()[0].mask().unwrap();
            assert!(mask.enabled());
            assert_eq!(mask.coverage().digest(), mask_digest);
            assert_eq!(mask.coverage().storage_usage(), storage);
            assert_eq!((doc.document_id, doc.state_id), identity);
            assert_eq!(doc.layers[&id].raster, raster);
            assert_eq!(doc.layers[&id].raster.resident_tiles(), 0);
            assert_eq!(
                doc.layers[&id]
                    .raster
                    .tile_versions(doc.canvas_rect())
                    .unwrap()
                    .collect::<Vec<_>>(),
                versions
            );
        }
    }
}
#[test]
fn literal_threshold_gpu_display_filters_before_nonlinear_reduction() {
    let Some(adapter) = adapter() else { return };
    for single in [false, true] {
        let gpu = renderer(&adapter, single);
        let mut doc = paintcraft_document::import_image(
            2,
            1,
            &[255, 0, 0, 255, 0, 255, 0, 255],
            "gpu-threshold-before-reduction/v1",
        )
        .unwrap();
        let id = doc.roots[0];
        let identity = (doc.document_id, doc.state_id);
        let raster = doc.layers[&id].raster.clone();
        let versions: Vec<_> = raster.tile_versions(doc.canvas_rect()).unwrap().collect();
        assert_eq!(
            versions.len(),
            1,
            "this control must exercise a real dense source tile"
        );
        doc.layers.get_mut(&id).unwrap().filters = stack(
            Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(128).unwrap()),
            None,
        );
        let full = gpu.render_with_stats(&doc, || Ok(())).unwrap();
        assert_eq!(full.pixels, [0, 0, 0, 255, 255, 255, 255, 255]);
        let warm = gpu.render_with_stats(&doc, || Ok(())).unwrap();
        assert!(warm.stats.source_hits > 0);
        assert_eq!(warm.pixels, full.pixels);
        for (level, expected) in [
            (0, vec![0, 0, 0, 255, 255, 255, 255, 255]),
            (1, vec![128, 128, 128, 255]),
        ] {
            let request = DisplayRequest::new(&doc, doc.canvas_rect(), level).unwrap();
            let output = gpu
                .render_display_texture(&doc, request, || Ok(()))
                .unwrap();
            assert_eq!(
                output.readback(|| Ok(())).unwrap(),
                expected,
                "single={single}, level={level}"
            );
            if level == 0 {
                assert_eq!(output.stats.single_layer_displays, u64::from(single));
            }
        }
        // Threshold(raw average RED/GREEN) would be black: its luma is118.7584.
        // Correct order averages the already filtered black/white pixels to128.
        let old = doc.layers[&id].filters.as_slice()[0].clone();
        doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![old.with_descriptor(
            Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(0).unwrap()),
        )])
        .unwrap();
        let changed = gpu.render_with_stats(&doc, || Ok(())).unwrap();
        assert!(changed.stats.source_allocations > 0);
        assert_eq!(changed.pixels, [255; 8]);
        let request = DisplayRequest::new(&doc, doc.canvas_rect(), 1).unwrap();
        assert_eq!(
            gpu.render_display_texture(&doc, request, || Ok(()))
                .unwrap()
                .readback(|| Ok(()))
                .unwrap(),
            [255; 4]
        );
        assert!((doc.layers[&id].filters.as_slice()[0]).mask().is_none());
        assert_eq!((doc.document_id, doc.state_id), identity);
        assert_eq!(doc.layers[&id].raster, raster);
        assert_eq!(doc.layers[&id].raster.get(0, 0), Rgba([255, 0, 0, 255]));
        assert_eq!(doc.layers[&id].raster.get(1, 0), Rgba([0, 255, 0, 255]));
        assert_eq!(
            doc.layers[&id]
                .raster
                .tile_versions(doc.canvas_rect())
                .unwrap()
                .collect::<Vec<_>>(),
            versions
        );
    }
}
