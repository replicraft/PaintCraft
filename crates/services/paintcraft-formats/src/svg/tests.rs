use super::*;
use std::{cell::Cell, io::Write};
fn options(width: u32, height: u32) -> Options {
    Options {
        width,
        height,
        dpi: 96.0,
        fit: Fit::Contain,
    }
}
fn svg(body: &str) -> Vec<u8> {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16">{body}</svg>"#)
        .into_bytes()
}
fn render(bytes: &[u8]) -> Result<Raster> {
    prepare(
        bytes,
        options(32, 16),
        &Fonts::default(),
        PIXELS_MAX,
        &|| Ok(()),
    )?
    .render(&|| Ok(()))
}

#[test]
fn explicit_size_aspect_fit_and_stretch_keep_alpha_straight() {
    let bytes = svg(r##"<rect width="32" height="16" fill="#ff0000" fill-opacity="0.5"/>"##);
    let prepared = prepare(
        &bytes,
        options(64, 64),
        &Fonts::default(),
        PIXELS_MAX,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(prepared.intrinsic_size(), (32.0, 16.0));
    let raster = prepared.render(&|| Ok(())).unwrap();
    assert_eq!(raster.losses, ["svg-rasterized"]);
    for y in 0..64 {
        for x in 0..64 {
            assert_eq!(
                &raster.rgba[(y * 64 + x) * 4..(y * 64 + x + 1) * 4],
                if (16..48).contains(&y) {
                    &[255, 0, 0, 128]
                } else {
                    &[0; 4]
                }
            );
        }
    }
    let stretched = prepare(
        &bytes,
        Options {
            fit: Fit::Stretch,
            ..options(64, 64)
        },
        &Fonts::default(),
        PIXELS_MAX,
        &|| Ok(()),
    )
    .unwrap()
    .render(&|| Ok(()))
    .unwrap();
    assert!(
        stretched
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [255, 0, 0, 128])
    );
}
#[test]
fn dpi_resolves_physical_units_and_invalid_options_fail_before_parsing() {
    let bytes = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1in" height="0.5in"><rect width="1" height="1"/></svg>"#;
    for dpi in [72.0, 96.0, 192.0, 300.0] {
        let prepared = prepare(
            bytes,
            Options {
                dpi,
                ..options(96, 48)
            },
            &Fonts::default(),
            5000,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(prepared.intrinsic_size(), (dpi, dpi * 0.5));
    }
    for invalid in [
        options(0, 16),
        options(1, 0),
        options(16385, 1),
        options(4096, 4096),
        Options {
            dpi: f32::NAN,
            ..options(1, 1)
        },
        Options {
            dpi: 2401.0,
            ..options(1, 1)
        },
    ] {
        assert!(
            prepare(
                b"invalid XML",
                invalid,
                &Fonts::default(),
                PIXELS_MAX,
                &|| Ok(())
            )
            .is_err()
        );
    }
}
#[test]
fn gzip_is_bounded_checked_and_never_accepts_a_suffix_or_second_member() {
    let original = svg("<rect width='32' height='16'/>");
    let gzip = |raw: &[u8]| {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(raw).unwrap();
        encoder.finish().unwrap()
    };
    let bytes = gzip(&original);
    assert_eq!(render(&bytes).unwrap(), render(&original).unwrap());
    for end in 0..bytes.len() {
        assert!(render(&bytes[..end]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(render(&trailing).is_err());
    let mut multiple = bytes.clone();
    multiple.extend(&bytes);
    assert!(render(&multiple).is_err());
    let mut corrupt = bytes;
    let len = corrupt.len();
    corrupt[len - 8] ^= 1;
    assert!(render(&corrupt).is_err());
    assert!(render(&gzip(&vec![b' '; SOURCE_MAX + 1])).is_err());
}
#[test]
fn external_resources_and_active_content_are_refused_before_rendering() {
    assert!(render(br#"<?xml version="1.0" encoding="ISO-8859-1"?><svg xmlns="http://www.w3.org/2000/svg"/>"#).is_err());
    for body in [
        "<!DOCTYPE svg [<!ENTITY x 'oops'>]>",
        "<script>alert(1)</script>",
        "<foreignObject/>",
        "<animate/>",
        "<rect onload='anything'/>",
        "<g xml:base='file:///tmp/'/>",
        "<image href='/etc/passwd'/>",
        "<image href='https://example.invalid/image.png'/>",
        "<image href='data:image/svg+xml;base64,PHN2Zy8+'/>",
        "<rect fill='url(https://example.invalid/a)'/>",
        "<rect fill='color(display-p3 1 0 0)'/>",
        "<rect fill='var(--paint)'/>",
        "<style>@import 'https://example.invalid/style.css';</style>",
        r"<style>.x { fill:u\72l(https://example.invalid/a) }</style>",
        "<style>.x { fill:URL(/*comment*/'https://example.invalid/a') }</style>",
        "<mystery/>",
        "<?xml-stylesheet href='file:///tmp/a.css'?>",
    ] {
        assert!(render(&svg(body)).is_err(), "accepted {body}");
    }
    assert!(
        render(
            br#"<!DOCTYPE svg SYSTEM "file:///tmp/a"><svg xmlns="http://www.w3.org/2000/svg"/>"#
        )
        .is_err()
    );
}
#[test]
fn references_are_local_unique_acyclic_and_expansion_is_bounded() {
    assert_eq!(
        render(&svg(&"<rect/>".repeat(16384))).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let good = svg("<defs><rect id='tile' width='4' height='4'/></defs><use href='#tile'/>");
    assert_eq!(&render(&good).unwrap().rgba[..4], &[0, 0, 0, 255]);
    for body in [
        "<use href='#missing'/>",
        "<g id='a'/><g id='a'/>",
        "<g id='a'><use href='#a'/></g>",
        "<image href='#a'/><g id='a'/>",
    ] {
        assert!(render(&svg(body)).is_err(), "accepted {body}");
    }
    let mut exponential = "<defs><g id='g0'><rect width='1' height='1'/></g>".to_owned();
    for i in 1..18 {
        exponential.push_str(&format!(
            "<g id='g{i}'><use href='#g{}'/><use href='#g{}'/></g>",
            i - 1,
            i - 1
        ));
    }
    exponential.push_str("</defs><use href='#g17'/>");
    assert_eq!(
        render(&svg(&exponential)).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let nested = format!(
        "{}<rect width='1' height='1'/>{}",
        "<g>".repeat(65),
        "</g>".repeat(65)
    );
    assert!(render(&svg(&nested)).is_err());
    // Earlier definitions are memoized at shallow depth. Reusing one under a
    // deep later branch must still charge its full remaining reference height.
    let mut chain = "<defs><rect id='c0' width='1' height='1'/>".to_owned();
    for i in 1..28 {
        chain.push_str(&format!("<g id='c{i}'><use href='#c{}'/></g>", i - 1));
    }
    chain.push_str("</defs>");
    chain.push_str(&"<g>".repeat(20));
    chain.push_str("<use href='#c27'/>");
    chain.push_str(&"</g>".repeat(20));
    assert!(render(&svg(&chain)).is_err());
}
#[test]
fn embedded_raster_is_validated_and_its_pixels_are_rendered() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let png =
        crate::image::encode(crate::image::ImageFormat::Png, 1, 1, &[255, 0, 0, 128], 100).unwrap();
    let body = format!(
        "<image width='32' height='16' preserveAspectRatio='none' image-rendering='pixelated' href='data:image/png;base64,{}'/>",
        STANDARD.encode(&png)
    );
    let raster = render(&svg(&body)).unwrap();
    assert!(
        raster
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [255, 0, 0, 128])
    );
    assert!(render(&svg(&body.replace("image/png", "image/jpeg"))).is_err());
    assert!(render(&svg("<image href='data:image/png;base64,AAAA'/>")).is_err());
}
#[test]
fn explicit_fonts_render_text_report_substitution_and_refuse_missing_glyphs() {
    let bytes = svg("<text x='0' y='14' font-size='14' font-family='Selawik'>Paint</text>");
    assert!(render(&bytes).is_err());
    let fonts = Fonts::from_data(
        &[include_bytes!(
            "../../../../../assets/fonts/selawik/selawk.ttf"
        )],
        "Selawik",
        &|| Ok(()),
    )
    .unwrap();
    let text = prepare(&bytes, options(32, 16), &fonts, PIXELS_MAX, &|| Ok(()))
        .unwrap()
        .render(&|| Ok(()))
        .unwrap();
    assert!(
        text.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] > 0)
    );
    assert_eq!(text.losses, ["svg-rasterized"]);
    let replaced = svg("<text x='0' y='14' font-family='Missing Font'>Paint</text>");
    assert!(
        prepare(&replaced, options(32, 16), &fonts, PIXELS_MAX, &|| Ok(()))
            .unwrap()
            .losses()
            .contains(&"svg-font-substituted".into())
    );
    assert!(
        prepare(
            &svg("<text>日本語</text>"),
            options(32, 16),
            &fonts,
            PIXELS_MAX,
            &|| Ok(())
        )
        .is_err()
    );
    assert!(Fonts::from_data(&[b"bad font"], "Selawik", &|| Ok(())).is_err());
}
#[test]
fn cancellation_at_every_prepare_and_output_checkpoint_returns_no_partial_pixels() {
    let bytes = svg("<rect width='32' height='16'/>");
    let total = Cell::new(0);
    let count = || {
        total.set(total.get() + 1);
        Ok(())
    };
    prepare(
        &bytes,
        options(32, 16),
        &Fonts::default(),
        PIXELS_MAX,
        &count,
    )
    .unwrap()
    .render(&count)
    .unwrap();
    for stop in 1..=total.get() {
        let current = Cell::new(0);
        let check = || {
            current.set(current.get() + 1);
            if current.get() == stop {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel SVG"))
            } else {
                Ok(())
            }
        };
        let result = prepare(
            &bytes,
            options(32, 16),
            &Fonts::default(),
            PIXELS_MAX,
            &check,
        )
        .and_then(|p| p.render(&check));
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(current.get(), stop);
    }
}
#[test]
fn expensive_filter_parameters_are_refused_before_rendering() {
    for primitive in [
        "<feTurbulence numOctaves='100000'/>",
        "<feConvolveMatrix order='1000'/>",
        "<feMorphology radius='100000'/>",
        "<feGaussianBlur stdDeviation='100000'/>",
    ] {
        let body = format!(
            "<defs><filter id='f'>{primitive}</filter></defs><rect width='32' height='16' filter='url(#f)'/>"
        );
        assert!(render(&svg(&body)).is_err(), "accepted {primitive}");
    }
}
