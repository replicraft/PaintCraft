use super::*;
use crate::{fonts, i18n::Language};

#[test]
fn selected_font_itself_parses_and_draws_without_fallback() {
    let ctx = egui::Context::default();
    let mut definitions = egui::FontDefinitions::default();
    definitions
        .font_data
        .insert(DATA_NAME.into(), egui::FontData::from_static(W95FA).into());
    definitions
        .families
        .insert(retro_family(), vec![DATA_NAME.into()]);
    ctx.set_fonts(definitions);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.fonts_mut(|fonts| {
            let id = egui::FontId::new(16.0, retro_family());
            assert!(fonts.has_glyphs(&id, "PaintCraft File Edit View 0123456789"));
            let galley = fonts.layout_no_wrap("PaintCraft".into(), id, egui::Color32::WHITE);
            assert!(galley.size().x > 0.0);
            assert!(
                galley
                    .rows
                    .iter()
                    .any(|row| !row.row.visuals.mesh.vertices.is_empty())
            );
        });
    });
    output.textures_delta.clear();
}

#[test]
fn locale_reinstallation_preserves_defaults_and_named_fallback_order() {
    let ctx = egui::Context::default();
    let styles = ctx.global_style().text_styles.clone();
    for language in Language::ALL {
        let locale = language.id();
        let definitions = super::super::definitions(locale);
        let cjk = if locale.starts_with("zh") {
            ["Noto Sans SC", "Noto Sans JP"]
        } else {
            ["Noto Sans JP", "Noto Sans SC"]
        };
        let old = egui::FontDefinitions::default();
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let mut expected = old.families[&family].clone();
            if family == egui::FontFamily::Proportional {
                expected.insert(0, "Selawik".into());
            }
            expected.push("Noto Sans Devanagari".into());
            expected.extend(cjk.map(str::to_owned));
            assert_eq!(
                definitions.families[&family], expected,
                "{locale} {family:?}"
            );
        }
        let named = &definitions.families[&retro_family()];
        assert_eq!(named[0], DATA_NAME);
        assert_eq!(
            &named[1..],
            definitions.families[&egui::FontFamily::Proportional]
        );
        fonts::install_for_locale(&ctx, locale);
        assert_eq!(ctx.global_style().text_styles, styles);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.fonts_mut(|fonts| {
                let id = egui::FontId::new(16.0, retro_family());
                for sample in [
                    "日本語 設定",
                    "简体中文 设置",
                    "हिन्दी भाषा",
                    "ÄÖÜ español português",
                ] {
                    assert!(fonts.has_glyphs(&id, sample), "{locale}: {sample}");
                    let galley =
                        fonts.layout_no_wrap(sample.into(), id.clone(), egui::Color32::WHITE);
                    assert!(
                        galley
                            .rows
                            .iter()
                            .any(|row| !row.row.visuals.mesh.vertices.is_empty())
                    );
                }
                let missing: std::collections::BTreeSet<_> =
                    crate::i18n::catalogs::sources(language)
                        .iter()
                        .flat_map(|source| source.chars())
                        .filter(|ch| !ch.is_control() && !fonts.has_glyph(&id, *ch))
                        .collect();
                assert!(missing.is_empty(), "{locale}: {missing:?}");
            });
        });
        output.textures_delta.clear();
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn interface_font_does_not_enter_the_document_collection() {
    let data = fonts::bundled_data();
    let expected = [
        fonts::SELAWIK_REGULAR,
        fonts::SELAWIK_BOLD,
        fonts::NOTO_SANS_JP,
        fonts::NOTO_SANS_SC,
        fonts::NOTO_SANS_DEVANAGARI,
    ];
    assert_eq!(data.len(), 5);
    for (actual, original) in data.into_iter().zip(expected) {
        assert!(std::ptr::eq(actual, original));
        assert_ne!(actual, W95FA);
    }
    assert_eq!(
        fonts::BUNDLED_FAMILIES,
        [
            "Noto Sans Devanagari",
            "Noto Sans JP",
            "Noto Sans SC",
            "Selawik"
        ]
    );
}
