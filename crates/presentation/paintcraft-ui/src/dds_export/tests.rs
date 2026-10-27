use super::*;
use crate::i18n::{Language, set_language, translate};
fn text(shape: &egui::epaint::Shape, labels: &mut Vec<String>) {
    match shape {
        egui::epaint::Shape::Text(shape) => labels.push(shape.galley.text().to_owned()),
        egui::epaint::Shape::Vec(shapes) => {
            for shape in shapes {
                text(shape, labels);
            }
        }
        _ => {}
    }
}
fn render(settings: &mut Settings, language: Language, size: (u32, u32)) -> (bool, Vec<String>) {
    let ctx = egui::Context::default();
    set_language(&ctx, language);
    let mut changed = false;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000., 1000.),
            )),
            ..Default::default()
        },
        |ui| {
            changed |= controls(ui, settings, size.0, size.1);
        },
    );
    output.textures_delta.clear();
    let mut labels = Vec::new();
    for shape in output.shapes {
        text(&shape.shape, &mut labels);
    }
    (changed, labels)
}
fn visible(labels: &[String], lang: Language, source: &str) -> bool {
    labels.iter().any(|s| s == &translate(lang, source))
}
#[test]
fn defaults_preserve_base_and_geometry_counts_include_the_original() {
    let s = Settings::default();
    assert_eq!(s.encoding, Encoding::Rgba8);
    assert_eq!(s.transfer, Transfer::Srgb);
    assert_eq!(s.mips, Mips::None);
    assert_eq!(s.filter, Filter::Nearest);
    assert_eq!(s.quality, Quality::Normal);
    for (w, h, max) in [(1, 1, 1), (17, 19, 5), (4096, 1, 13), (1, 65536, 17)] {
        assert_eq!(maximum_levels(w, h), max);
        assert_eq!(s.level_count(w, h), Some(1));
        assert_eq!(
            Settings {
                mips: Mips::Full,
                ..s
            }
            .level_count(w, h),
            Some(max)
        );
        assert_eq!(
            Settings {
                mips: Mips::Custom(max + 1),
                ..s
            }
            .level_count(w, h),
            None
        );
    }
    assert_eq!(s.level_count(0, 3), None);
}
#[test]
fn actual_english_and_japanese_controls_hide_inactive_effort_and_filters() {
    for lang in [Language::English, Language::Japanese] {
        let mut s = Settings::default();
        let (changed, labels) = render(&mut s, lang, (17, 19));
        assert!(!changed);
        assert!(visible(&labels, lang, "DDS encoding"));
        assert!(visible(&labels, lang, "RGBA8 (uncompressed)"));
        assert!(visible(&labels, lang, "sRGB samples"));
        assert!(!visible(&labels, lang, "Encoding effort"));
        assert!(!visible(&labels, lang, "Mip filter"));
        s.encoding = Encoding::Bc1;
        s.mips = Mips::Full;
        s.transfer = Transfer::Linear8;
        let (_, labels) = render(&mut s, lang, (17, 19));
        for source in [
            "Encoding effort",
            "Mip filter",
            "BC1 (opaque)",
            "Linear 8-bit samples",
            "BC1 changes colors and refuses pictures with transparency. No background is added.",
            "Linear 8-bit converts sRGB samples and can lose color precision. Transparency is unchanged by this conversion.",
        ] {
            assert!(
                visible(&labels, lang, source),
                "{} missing {source}: {labels:?}",
                lang.id()
            );
        }
        for encoding in [Encoding::Bc2, Encoding::Bc3] {
            s.encoding = encoding;
            let (_, labels) = render(&mut s, lang, (17, 19));
            assert!(visible(
                &labels,
                lang,
                "BC2 and BC3 store straight alpha. Compression can change color and transparency."
            ));
        }
    }
}
#[test]
fn geometry_changes_clamp_custom_levels_and_report_changed_settings() {
    for lang in [Language::English, Language::Japanese] {
        let mut s = Settings {
            mips: Mips::Custom(17),
            quality: Quality::High,
            ..Default::default()
        };
        let (changed, labels) = render(&mut s, lang, (17, 19));
        assert!(changed);
        assert_eq!(s.mips, Mips::Custom(5));
        assert_eq!(s.quality, Quality::Normal);
        assert!(visible(&labels, lang, "Level count (including base image)"));
        assert!(!render(&mut s, lang, (17, 19)).0);
        assert!(render(&mut s, lang, (1, 1)).0);
        assert_eq!(s.mips, Mips::Custom(1));
        let (_, labels) = render(&mut s, lang, (1, 1));
        assert!(!visible(&labels, lang, "Mip filter"));
        s.mips = Mips::Custom(0);
        assert!(render(&mut s, lang, (7, 5)).0);
        assert_eq!(s.mips, Mips::Custom(1));
        let (_, labels) = render(&mut s, lang, (0, 1));
        assert!(visible(
            &labels,
            lang,
            "DDS export requires a nonempty image."
        ));
        assert_eq!(s.level_count(0, 1), None);
    }
}
#[test]
fn labels_and_honest_conversion_warnings_are_translated_in_all_seven_catalogs() {
    for lang in Language::ALL {
        if lang == Language::English {
            continue;
        }
        for source in [
            "DDS encoding",
            encoding_label(Encoding::Rgba8),
            encoding_label(Encoding::Bc1),
            encoding_label(Encoding::Bc2),
            encoding_label(Encoding::Bc3),
            "DDS color samples",
            "sRGB samples",
            "Linear 8-bit samples",
            "Mip levels",
            mip_label(Mips::None),
            mip_label(Mips::Full),
            mip_label(Mips::Custom(2)),
            "Level count (including base image)",
            "Mip filter",
            filter_label(Filter::Nearest),
            filter_label(Filter::BoxStored),
            "Encoding effort",
            quality_label(Quality::Normal),
            quality_label(Quality::High),
            "RGBA8 stores each pixel without block compression.",
            "BC1 changes colors and refuses pictures with transparency. No background is added.",
            "BC2 and BC3 store straight alpha. Compression can change color and transparency.",
            "sRGB keeps the document's stored color samples.",
            "Linear 8-bit converts sRGB samples and can lose color precision. Transparency is unchanged by this conversion.",
            "Smaller mip images are derived from the base. Filtering uses the chosen stored samples.",
            "Higher effort takes longer. It does not guarantee an error limit or lossless compression.",
            "DDS export requires a nonempty image.",
        ] {
            assert_ne!(translate(lang, source), source, "{}: {source}", lang.id());
        }
    }
}
