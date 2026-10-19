//! Independent pixel arithmetic, including every source alpha and clipped layers.
use paintcraft_core::types::Rgba;
use paintcraft_document::{POLICY, new_document};
use paintcraft_raster::{Admission, BlendMode};
use paintcraft_render::render_flat;

fn over(source: [u8; 4], dest: [u8; 4], opacity: u8) -> [u8; 4] {
    let a = (u64::from(source[3]) * u64::from(opacity) + 127) / 255;
    if a == 0 {
        return dest;
    }
    let b = u64::from(dest[3]);
    let denominator = a * 255 + b * (255 - a);
    let mut out = [0; 4];
    for c in 0..3 {
        let numerator = u64::from(source[c]) * a * 255 + u64::from(dest[c]) * b * (255 - a);
        out[c] = ((numerator + denominator / 2) / denominator) as u8;
    }
    out[3] = ((denominator + 127) / 255) as u8;
    out
}

#[test]
fn every_alpha_and_opacity_boundary_matches_an_independent_per_pixel_compositor() {
    for alpha in 0..=255 {
        let mut doc = new_document(17, 9, Some(Rgba([12, 34, 56, alpha])), 5, POLICY).unwrap();
        let template = doc.layers[&doc.roots[0]].clone();
        let mut adm = Admission::new(16 * 65536);
        for (n, opacity) in [0, 1, 127, 128, 254, 255].into_iter().enumerate() {
            let mut layer = template.clone();
            layer.id =
                paintcraft_core::types::ids::deterministic_id(&["independent", &n.to_string()]);
            layer.origin = (n as i64 * 5 - 12, n as i64 - 3);
            layer.opacity = opacity;
            layer.visible = n != 1;
            layer
                .raster
                .blend_pixel(
                    4,
                    5,
                    Rgba([255, 76, 43, 255 - alpha]),
                    BlendMode::Replace,
                    &mut adm,
                )
                .unwrap();
            doc.roots.push(layer.id);
            doc.layers.insert(layer.id, layer);
        }
        let mut expected = Vec::new();
        for y in 0..9 {
            for x in 0..17 {
                let mut pixel = [0; 4];
                for layer in doc.layers_in_order().filter(|layer| layer.visible) {
                    if layer.extent().contains(x, y) {
                        let src = layer.raster.get(x - layer.origin.0, y - layer.origin.1);
                        pixel = over(src.0, pixel, layer.opacity);
                    }
                }
                expected.extend_from_slice(&pixel);
            }
        }
        let ids: Vec<_> = doc
            .layers_in_order()
            .flat_map(|l| l.raster.tile_identities())
            .collect();
        let first = render_flat(&doc).unwrap();
        assert_eq!(first, expected, "alpha={alpha}");
        assert_eq!(render_flat(&doc).unwrap(), first);
        assert_eq!(
            doc.layers_in_order()
                .flat_map(|l| l.raster.tile_identities())
                .collect::<Vec<_>>(),
            ids
        );
    }
}

#[test]
fn transparent_hidden_rgb_and_partial_edge_tiles_do_not_leak_into_flat_output() {
    let mut doc = new_document(259, 257, Some(Rgba([12, 34, 56, 0])), 6, POLICY).unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer
        .raster
        .blend_pixel(
            258,
            256,
            Rgba([9, 8, 7, 255]),
            BlendMode::Replace,
            &mut Admission::new(65536),
        )
        .unwrap();
    let output = render_flat(&doc).unwrap();
    assert!(output[..output.len() - 4].iter().all(|byte| *byte == 0));
    assert_eq!(&output[output.len() - 4..], [9, 8, 7, 255]);
}
