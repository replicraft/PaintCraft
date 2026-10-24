//! Bundled interface typography, independent of the host's installed fonts.

pub const SELAWIK_REGULAR: &[u8] = include_bytes!("../../../../assets/fonts/selawik/selawk.ttf");
pub const SELAWIK_BOLD: &[u8] = include_bytes!("../../../../assets/fonts/selawik/selawkb.ttf");
pub const NOTO_SANS_JP: &[u8] =
    include_bytes!("../../../../assets/fonts/noto-sans-jp/NotoSansJP-Regular.otf");
pub const NOTO_SANS_SC: &[u8] =
    include_bytes!("../../../../assets/fonts/noto-sans-sc/NotoSansSC-Regular.otf");
pub const NOTO_SANS_DEVANAGARI: &[u8] =
    include_bytes!("../../../../assets/fonts/noto-sans-devanagari/NotoSansDevanagari.ttf");

/// The same portable font collection is used for interface and document text.
pub const BUNDLED_DATA: [&[u8]; 5] = [
    SELAWIK_REGULAR,
    SELAWIK_BOLD,
    NOTO_SANS_JP,
    NOTO_SANS_SC,
    NOTO_SANS_DEVANAGARI,
];
pub const BUNDLED_FAMILIES: [&str; 4] = [
    "Noto Sans Devanagari",
    "Noto Sans JP",
    "Noto Sans SC",
    "Selawik",
];

/// Use Selawik for proportional UI text and bundled fallback for non-Latin scripts.
pub fn install(ctx: &egui::Context) {
    install_for_locale(ctx, "en");
}

/// Prefer the locale's CJK forms while keeping every language available in menus.
/// Call on initialization or a language change, since replacing fonts resets layout caches.
pub fn install_for_locale(ctx: &egui::Context, locale: &str) {
    ctx.set_fonts(definitions(locale));
}

fn definitions(locale: &str) -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in [
        ("Selawik", SELAWIK_REGULAR),
        ("Noto Sans JP", NOTO_SANS_JP),
        ("Noto Sans SC", NOTO_SANS_SC),
        ("Noto Sans Devanagari", NOTO_SANS_DEVANAGARI),
    ] {
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_static(bytes).into());
    }
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Selawik".into());
    let cjk = if locale.starts_with("zh") {
        ["Noto Sans SC", "Noto Sans JP"]
    } else {
        ["Noto Sans JP", "Noto Sans SC"]
    };
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let fallback = fonts.families.entry(family).or_default();
        fallback.push("Noto Sans Devanagari".into());
        fallback.extend(cjk.map(str::to_owned));
    }
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_fonts_cover_all_locale_scripts_without_system_fonts() {
        let ctx = egui::Context::default();
        install(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.fonts_mut(|fonts| {
                for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                    let id = egui::FontId::new(16.0, family);
                    for sample in [
                        "English Deutsch Español Português ÄÖÜäöüßáéíóúñãõç",
                        "日本語 設定 色 選択",
                        "简体中文 设置 颜色 选择",
                        "हिन्दी भाषा विकल्प पेंसिल आकार",
                    ] {
                        assert!(fonts.has_glyphs(&id, sample), "Missing glyph in {sample}");
                    }
                }
            });
        });
        output.textures_delta.clear();
    }

    #[test]
    fn bundled_fallback_covers_every_localized_catalog_character() {
        for language in crate::i18n::Language::ALL {
            let ctx = egui::Context::default();
            install_for_locale(&ctx, language.id());
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.fonts_mut(|fonts| {
                    let id = egui::FontId::proportional(16.0);
                    let missing: std::collections::BTreeSet<_> =
                        crate::i18n::catalogs::sources(language)
                            .iter()
                            .flat_map(|source| source.chars())
                            .filter(|ch| !ch.is_control() && !fonts.has_glyph(&id, *ch))
                            .collect();
                    assert!(
                        missing.is_empty(),
                        "{} missing glyphs: {missing:?}",
                        language.id()
                    );
                });
            });
            output.textures_delta.clear();
        }
    }

    #[test]
    fn devanagari_conjunct_is_shaped_instead_of_independent_characters() {
        let ctx = egui::Context::default();
        install(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let shaped = ui.fonts_mut(|fonts| {
                fonts.layout_no_wrap(
                    "क्ष".into(),
                    egui::FontId::proportional(24.0),
                    egui::Color32::WHITE,
                )
            });
            let glyphs = &shaped.rows[0].row.glyphs;
            // egui retains zero-width continuation entries for cursor positions.
            assert_eq!(
                glyphs
                    .iter()
                    .filter(|glyph| glyph.advance_width > 0.0)
                    .count(),
                1,
                "Devanagari conjunct must form one advancing glyph"
            );
            assert!(glyphs[0].advance_width > 0.0);
            assert!(shaped.rows[0].row.visuals.mesh.vertices.len() >= 4);
        });
        output.textures_delta.clear();
    }

    #[test]
    fn chinese_and_japanese_prefer_their_own_cjk_forms() {
        for (locale, first, second) in [
            ("zh-CN", "Noto Sans SC", "Noto Sans JP"),
            ("ja", "Noto Sans JP", "Noto Sans SC"),
        ] {
            let fonts = definitions(locale);
            let fallback = &fonts.families[&egui::FontFamily::Proportional];
            assert_eq!(fallback.first().unwrap(), "Selawik");
            assert!(
                fallback.iter().position(|name| name == first)
                    < fallback.iter().position(|name| name == second)
            );
        }
    }
}
