#![cfg(feature = "gpu")]
use paintcraft_core::types::{Rgba, ids::deterministic_id};
use paintcraft_document::{Document, Layer, POLICY, new_document};
use paintcraft_raster::{Admission, RasterLayer};
use paintcraft_render::{gpu::GpuRenderer, render_flat};

#[test]
fn gpu_matches_scalar_for_order_opacity_offsets_tiles_and_hidden_rgb() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
        Ok(a) => a,
        Err(e) => {
            assert!(
                std::env::var_os("PAINTCRAFT_REQUIRE_GPU").is_none(),
                "GPU required: {e}"
            );
            eprintln!("GPU unavailable; parity test skipped: {e}");
            return;
        }
    };
    eprintln!("GPU parity adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let mut doc = new_document(263, 19, None, 0, POLICY).unwrap();
    assert_eq!(gpu.render(&doc).unwrap(), render_flat(&doc).unwrap());
    // Odd extents exercise workgroup tails; width crosses the sparse tile boundary.
    for (i, (origin, opacity, visible)) in [
        ((-3, -1), 255, true),
        ((1, 2), 128, true),
        ((0, 0), 0, true),
        ((0, 0), 200, false),
        ((256, 0), 241, true),
        ((2000, 0), 255, true),
    ]
    .into_iter()
    .enumerate()
    {
        let (w, h) = (270, 23);
        let bytes: Vec<u8> = (0..w * h)
            .flat_map(|p| {
                let n = (p + i as u32 * 53) as u8;
                [
                    n,
                    n.wrapping_mul(17),
                    255 - n,
                    if p % 7 == 0 { 0 } else { n },
                ]
            })
            .collect();
        let raster = RasterLayer::from_rgba(
            w,
            h,
            Rgba([91, 37, 11, 0]),
            &bytes,
            &mut Admission::new(2_000_000),
        )
        .unwrap();
        let id = deterministic_id(&["gpu-test", &i.to_string()]);
        doc.roots.push(id);
        doc.layers.insert(
            id,
            Layer {
                id,
                name: format!("Layer {i}"),
                origin,
                visible,
                opacity,
                raster,
            },
        );
        parity(&gpu, &doc);
    }
    doc.roots.reverse();
    parity(&gpu, &doc);
    // A transparent sparse default is different from a missing layer, and must stay harmless.
    let id = doc.roots[0];
    doc.layers.get_mut(&id).unwrap().raster = RasterLayer::new(270, 23, Rgba([123, 45, 67, 0]));
    parity(&gpu, &doc);
    let too_large = new_document(16384, 16384, None, 0, POLICY).unwrap();
    assert!(gpu.render(&too_large).is_err());
}
fn parity(gpu: &GpuRenderer, doc: &Document) {
    assert_eq!(
        gpu.render(doc).unwrap(),
        render_flat(doc).unwrap(),
        "GPU must exactly preserve integer compositing policy"
    );
}
