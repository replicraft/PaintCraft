//! Native font discovery and bundled font wiring for the shared text rasterizer.
pub(crate) use paintcraft_core::types::text::TextMask;
pub(crate) use paintcraft_tools::text::TextRenderer;
use paintcraft_tools::text::{Database, FontSystem};
use std::sync::{Arc, Mutex};
pub(crate) type Shared = Arc<Mutex<TextRenderer>>;

fn load_bundled(db: &mut Database) {
    for bytes in paintcraft_ui::fonts::BUNDLED_DATA {
        db.load_font_data(bytes.to_vec());
    }
    db.set_sans_serif_family("Selawik");
}
pub(crate) fn bundled() -> TextRenderer {
    let mut db = Database::new();
    load_bundled(&mut db);
    TextRenderer::new(FontSystem::new_with_locale_and_db("en-US".into(), db))
}
pub(crate) fn system() -> TextRenderer {
    let mut fonts = FontSystem::new();
    load_bundled(fonts.db_mut());
    TextRenderer::new(fonts)
}
pub(crate) fn families(renderer: &TextRenderer) -> Vec<String> {
    paintcraft_platform::fonts::FontCatalog::from_database(renderer.database()).families
}

#[cfg(test)]
use paintcraft_core::types::{
    Rect,
    text::{TextAlignment, TextOptions},
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_document_text_rasterizes_every_locale_without_system_fonts() {
        let mut renderer = bundled();
        let families = families(&renderer);
        for family in paintcraft_ui::fonts::BUNDLED_FAMILIES {
            assert!(
                families.iter().any(|name| name == family),
                "Missing {family}"
            );
        }
        for text in [
            "Deutsch Español Português",
            "日本語",
            "简体中文",
            "हिन्दी क्ष",
        ] {
            // Request Latin Selawik to exercise script fallback, not an explicit Noto face.
            let style = TextOptions {
                family: "Selawik".into(),
                points: 24,
                ..Default::default()
            };
            let mask = renderer
                .rasterize(text, &style, (4, 4), Rect::new(0, 0, 500, 100).unwrap())
                .unwrap();
            assert!(!mask.spans.is_empty(), "Empty mask for {text}");
            assert!(mask.spans.iter().any(|span| span.coverage < 255));
            paintcraft_commands::validate_mask_spans(&mask.spans).unwrap();
        }
    }

    #[test]
    fn selawik_masks_have_grayscale_edges_real_styles_alignment_and_a_reused_cache() {
        let mut renderer = bundled();
        let clip = Rect::new(0, 0, 200, 100).unwrap();
        let mut style = TextOptions {
            points: 22,
            ..Default::default()
        };
        let regular = renderer.rasterize("Paint", &style, (5, 5), clip).unwrap();
        assert!(
            regular
                .spans
                .iter()
                .any(|s| s.coverage > 0 && s.coverage < 255)
        );
        assert!(Arc::ptr_eq(
            &regular,
            &renderer.rasterize("Paint", &style, (5, 5), clip).unwrap()
        ));
        style.bold = true;
        let bold = renderer.rasterize("Paint", &style, (5, 5), clip).unwrap();
        assert_ne!(regular.spans, bold.spans);
        style.italic = true;
        let italic = renderer.rasterize("Paint", &style, (5, 5), clip).unwrap();
        assert_ne!(bold.spans, italic.spans);
        style.underline = true;
        style.strikeout = true;
        let decorated = renderer.rasterize("Paint", &style, (5, 5), clip).unwrap();
        assert_ne!(italic.spans, decorated.spans);
        style.alignment = TextAlignment::Right;
        let right = renderer.rasterize("Paint", &style, (5, 5), clip).unwrap();
        assert!(
            right.spans.iter().map(|s| s.x0).min().unwrap()
                > regular.spans.iter().map(|s| s.x0).min().unwrap()
        );
        assert!(
            right
                .spans
                .iter()
                .all(|s| s.x0 >= 0 && s.x1 < 200 && s.y >= 0 && s.y < 100)
        );
    }
}

#[cfg(test)]
mod native_review {
    use super::*;
    #[test]
    #[ignore = "requires an installed font collection; native font/shaping review"]
    fn system_fonts_shape_and_rasterize_for_review() {
        let mut renderer = system();
        let families = families(&renderer);
        assert!(families.len() > 1);
        let family = if families.iter().any(|s| s == "Arial") {
            "Arial"
        } else {
            "Selawik"
        };
        let style = TextOptions {
            family: family.into(),
            points: 24,
            ..Default::default()
        };
        let mask = renderer
            .rasterize(
                "PaintCraft: café Ω مرحبا",
                &style,
                (10, 10),
                Rect::new(0, 0, 800, 200).unwrap(),
            )
            .unwrap();
        assert!(mask.spans.iter().any(|s| s.coverage < 255));
        paintcraft_commands::validate_mask_spans(&mask.spans).unwrap();
        eprintln!(
            "{} visible installed families; selected {family}; {} resolved spans",
            families.len(),
            mask.spans.len()
        );
    }
}
