//! Integration authority is source coordinates; numerical literals are independent.
use paintcraft_core::types::ids::deterministic_id;
use paintcraft_core::types::{
    ErrorCode, Rect, Rgba,
    coordinates::SourcePixel,
    filters::{AddNoiseRgbV1, BoxBlurV1, Descriptor, NoiseDistribution},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{Document, FilterInstance, FilterMask, FilterStack};
use paintcraft_render::{
    noise_work::{NoiseAdmission, NoiseJobPolicy, NoiseLimits, with_noise_workspace},
    render_region,
};
fn policy(d: NoiseDistribution) -> AddNoiseRgbV1 {
    AddNoiseRgbV1::new(100, 65535, false, d).unwrap()
}
fn scene(d: NoiseDistribution) -> Document {
    let mut doc = paintcraft_document::new_document(
        260,
        2,
        Some(Rgba([64, 16, 128, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["noise-coordinates", "1"]),
            true,
            Descriptor::AddNoiseRgbV1(policy(d)),
        )])
        .unwrap();
    doc
}
fn paired(b: &SharedBudget, noise: u64) -> NoiseAdmission {
    NoiseAdmission::new_root(
        b.scope(AllocationClass::SharedBudget),
        NoiseJobPolicy::BrowserV1,
        NoiseLimits {
            bytes: 64 << 20,
            base_work: 268_435_456,
            noise_work: noise,
        },
    )
    .unwrap()
}
fn render(doc: &Document, area: Rect) -> Vec<u8> {
    let b = SharedBudget::new(64 << 20);
    with_noise_workspace(paired(&b, 1_000_000_000), || render_region(doc, area)).unwrap()
}
#[test]
fn implicit_source_literal_field_and_fractional_mask_are_not_uniform() {
    // Frozen standalone independent Fraction/Decimal oracle, not the renderer.
    for (d, full, masked) in [
        (
            NoiseDistribution::Uniform,
            [255, 211, 0, 255, 51, 0, 97, 255],
            [159, 113, 64, 255],
        ),
        (
            NoiseDistribution::Gaussian,
            [47, 139, 0, 255, 0, 4, 46, 255],
            [56, 77, 64, 255],
        ),
    ] {
        let mut doc = scene(d);
        let before = doc.clone();
        assert_eq!(render(&doc, Rect::new(0, 0, 2, 1).unwrap()), full);
        assert_eq!(doc.layers[&doc.roots[0]].raster.resident_tiles(), 0);
        let b = SharedBudget::new(1 << 20);
        let mask = paintcraft_raster::coverage_mask::A8Mask::uniform(
            Rect::new(0, 0, 1, 1).unwrap(),
            127,
            &b,
            Default::default(),
            &|| Ok(()),
        )
        .unwrap();
        let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
        layer.filters = FilterStack::new(vec![
            layer.filters.as_slice()[0].with_mask(Some(FilterMask::new(true, mask))),
        ])
        .unwrap();
        assert_eq!(render(&doc, Rect::new(0, 0, 1, 1).unwrap()), masked);
        assert_eq!(
            render(&doc, Rect::new(1, 0, 1, 1).unwrap()),
            [64, 16, 128, 255]
        );
        assert_eq!(before.layers[&before.roots[0]].raster.resident_tiles(), 0);
    }
}
#[test]
fn tiled_roi_translation_and_source_tile_boundaries_share_absolute_coordinates() {
    for d in [NoiseDistribution::Uniform, NoiseDistribution::Gaussian] {
        let mut doc = scene(d);
        let before = doc.clone();
        let whole = render(&doc, doc.canvas_rect());
        for x in [0, 1, 15, 16, 63, 64, 255, 256, 259] {
            for y in 0..2 {
                let one = render(&doc, Rect::new(x, y, 1, 1).unwrap());
                let start = ((y * 260 + x) * 4) as usize;
                assert_eq!(one, whole[start..start + 4]);
                // Differential coordinate plumbing check, independent literals above.
                assert_eq!(
                    one,
                    paintcraft_effects::noise::evaluate_pixel(
                        policy(d),
                        SourcePixel::new(x as u32, y as u32),
                        Rgba([64, 16, 128, 255])
                    )
                    .unwrap()
                    .0
                );
            }
        }
        assert!(doc.same_semantics(&before));
        doc.layers.get_mut(&doc.roots[0]).unwrap().origin = (-3, -1);
        assert_eq!(
            render(&doc, Rect::new(60, 0, 2, 1).unwrap()),
            whole[(260 + 63) * 4..(260 + 65) * 4]
        );
    }
}
#[test]
fn nonzero_noise_box_order_evaluates_actual_finite_source_prefix() {
    let mut doc = scene(NoiseDistribution::Uniform);
    let id = doc.roots[0];
    let blur = FilterInstance::new(
        deterministic_id(&["noise-coordinates", "2"]),
        true,
        Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
    );
    let noise = doc.layers[&id].filters.as_slice()[0].clone();
    doc.layers.get_mut(&id).unwrap().filters =
        FilterStack::new(vec![noise.clone(), blur.clone()]).unwrap();
    let before = render(&doc, Rect::new(0, 0, 2, 1).unwrap());
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![blur, noise]).unwrap();
    let after = render(&doc, Rect::new(0, 0, 2, 1).unwrap());
    assert_ne!(before, after);
    assert_eq!(after, [255, 211, 0, 255, 51, 0, 97, 255]);
    // Tiled prefix halos must agree with the same whole ordered render.
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![
        FilterInstance::new(
            deterministic_id(&["noise-coordinates", "1"]),
            true,
            Descriptor::AddNoiseRgbV1(policy(NoiseDistribution::Uniform)),
        ),
        FilterInstance::new(
            deterministic_id(&["noise-coordinates", "2"]),
            true,
            Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        ),
    ])
    .unwrap();
    let whole = render(&doc, Rect::new(0, 0, 65, 2).unwrap());
    assert_eq!(
        render(&doc, Rect::new(63, 0, 2, 1).unwrap()),
        whole[63 * 4..65 * 4]
    );
}
#[test]
fn direct_work_is_policy_priced_and_zero_amount_still_requires_scope() {
    let mut doc = scene(NoiseDistribution::Uniform);
    let id = doc.roots[0];
    doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["noise-coordinates", "1"]),
        true,
        Descriptor::AddNoiseRgbV1(
            AddNoiseRgbV1::new(0, 1, true, NoiseDistribution::Uniform).unwrap(),
        ),
    )])
    .unwrap();
    let a = Rect::new(0, 0, 1, 1).unwrap();
    assert_eq!(
        render_region(&doc, a).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let b = SharedBudget::new(64 << 20);
    let scope = paired(&b, 1023);
    assert_eq!(
        with_noise_workspace(scope, || render_region(&doc, a))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(render(&doc, a), [64, 16, 128, 255]);
}

#[test]
fn cached_cold_warm_lod_and_parameter_only_edits_keep_exact_source_roi() {
    use paintcraft_render::{
        cache::{CacheLimits, CompositeCache, Domain},
        display::{DisplayCache, DisplayRequest},
    };
    fn limits() -> CacheLimits {
        CacheLimits {
            tile_edge: 64,
            committed_bytes: 1 << 20,
            preview_bytes: 1 << 20,
            entries_per_domain: 32,
        }
    }
    for (d, literal) in [
        (NoiseDistribution::Uniform, [153, 106, 49, 255]),
        (NoiseDistribution::Gaussian, [24, 72, 23, 255]),
    ] {
        let mut doc = scene(d);
        doc.canvas = (2, 1);
        let b = SharedBudget::new(64 << 20);
        let unit = paintcraft_effects::noise::quote(policy(d), 1).unwrap().work;
        let area = Rect::new(0, 0, 2, 1).unwrap();
        let request = DisplayRequest::new(&doc, area, 1).unwrap();
        let mut c = DisplayCache::new(limits()).unwrap();
        for _ in 0..2 {
            let a = paired(&b, unit * 4);
            let output = with_noise_workspace(a.clone(), || {
                c.render(&doc, request, Domain::Committed, || Ok(()))
            })
            .unwrap();
            assert_eq!(output.pixels, literal);
            assert!(a.used().unwrap().noise <= unit * 4);
        }
        let a = paired(&b, unit * 4 - 1);
        assert_eq!(
            with_noise_workspace(a.clone(), || c.render(
                &doc,
                request,
                Domain::Committed,
                || Ok(())
            ))
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(a.used().unwrap().noise, 0);
        let mut composite = CompositeCache::new(limits()).unwrap();
        let one = Rect::new(1, 0, 1, 1).unwrap();
        let a = paired(&b, unit * 2);
        let old = with_noise_workspace(a, || {
            composite.render(&doc, one, Domain::Committed, || Ok(()))
        })
        .unwrap()
        .pixels;
        let state = doc.state_id;
        let id = doc.roots[0];
        let layer = doc.layers.get_mut(&id).unwrap();
        layer.filters = FilterStack::new(vec![layer.filters.as_slice()[0].with_descriptor(
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(100, 0, false, d).unwrap()),
        )])
        .unwrap();
        assert_eq!(doc.state_id, state);
        assert_eq!(doc.layers[&id].raster.resident_tiles(), 0);
        let a = paired(&b, unit * 2);
        let new = with_noise_workspace(a, || {
            composite.render(&doc, one, Domain::Committed, || Ok(()))
        })
        .unwrap()
        .pixels;
        assert_ne!(new, old);
        assert_eq!(new, render(&doc, one));
    }
}
