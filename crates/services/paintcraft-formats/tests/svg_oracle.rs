//! Independent Chromium corpus. Exact aligned samples, measured AA differences.
use paintcraft_core::types::json;
use paintcraft_formats::{
    image,
    svg::{self, Fit, Fonts, Options},
};
use sha2::{Digest, Sha256};

#[test]
fn selected_svg_renderer_is_compared_to_independent_chromium_pixels() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/svg-fixtures");
    let oracle = json::parse(&std::fs::read(root.join("oracle.json")).unwrap()).unwrap();
    let font = include_bytes!("../../../../assets/fonts/selawik/selawk.ttf");
    assert_eq!(
        paintcraft_document::hex(&Sha256::digest(font)),
        oracle.get("font_sha256").unwrap().as_str().unwrap()
    );
    let fonts = Fonts::from_data(&[font], "Selawik", &|| Ok(())).unwrap();
    for fixture in oracle.get("files").unwrap().as_array().unwrap() {
        let name = fixture.get("name").unwrap().as_str().unwrap();
        let source = std::fs::read(root.join(format!("{name}.svg"))).unwrap();
        let png = std::fs::read(root.join(format!("{name}.chrome.png"))).unwrap();
        for (bytes, field) in [(&source, "source_sha256"), (&png, "png_sha256")] {
            assert_eq!(
                paintcraft_document::hex(&Sha256::digest(bytes)),
                fixture.get(field).unwrap().as_str().unwrap()
            );
        }
        let expected = image::decode(&png).unwrap();
        let options = Options {
            width: expected.width,
            height: expected.height,
            dpi: 96.0,
            fit: Fit::Contain,
        };
        let actual = svg::prepare(&source, options, &fonts, svg::PIXELS_MAX, &|| Ok(()))
            .unwrap()
            .render(&|| Ok(()))
            .unwrap();
        let mut sum = 0u64;
        let mut maximum = 0u16;
        let mut significant = 0u64;
        for (a, b) in actual
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .zip(expected.rgba.as_chunks::<4>().0)
        {
            let mut changed = false;
            for channel in 0..4 {
                // Premultiplied comparison avoids undefined RGB at zero alpha
                // and unpremultiplication amplification on faint AA edges.
                let premul = |p: &[u8; 4]| {
                    if channel == 3 {
                        u16::from(p[3])
                    } else {
                        (u16::from(p[channel]) * u16::from(p[3]) + 127) / 255
                    }
                };
                let delta = premul(a).abs_diff(premul(b));
                maximum = maximum.max(delta);
                sum += u64::from(delta);
                changed |= delta > 8;
            }
            significant += u64::from(changed);
        }
        let pixels = u64::from(expected.width) * u64::from(expected.height);
        let mean = sum as f64 / (pixels * 4) as f64;
        let fraction = significant as f64 / pixels as f64;
        println!("{name}: max={maximum}, mean={mean:.6}, pixels_over_8={fraction:.6}");
        if name == "aligned" || name == "embedded" {
            assert_eq!(actual.rgba, expected.rgba, "{name}");
        } else if name == "embedded-css-smooth" {
            // Chrome 155 does not implement CSS `smooth`; computed style stays
            // pixelated. Retain this discrepancy without calling it agreement.
            assert_eq!(
                fixture.get("image_rendering").unwrap().as_array().unwrap()[0].as_str(),
                Some("pixelated")
            );
            assert_ne!(actual.rgba, expected.rgba);
            assert_eq!(
                oracle.get("css_smooth_supported").unwrap().as_bool(),
                Some(false)
            );
        } else {
            // Explicitly a renderer comparison, not pixel-equivalence evidence.
            // Reject substantial shape/layout/color changes while allowing the
            // different rasterizers' antialiasing and font hinting.
            assert!(
                mean <= 2.0 && fraction <= 0.15,
                "{name}: max={maximum}, mean={mean}, changed={fraction}"
            );
        }
    }
}
