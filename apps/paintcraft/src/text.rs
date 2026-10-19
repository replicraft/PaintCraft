//! Desktop document text: shaping/fallback via Cosmic Text, grayscale Swash masks.
use cosmic_text::{
    Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache,
    UnderlineStyle, Weight,
};
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Result,
    text::{MaskSpan, TextAlignment, TextOptions},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
pub(crate) type Shared = Arc<Mutex<TextRenderer>>;
const PIXELS_MAX: usize = 4_194_304;
#[derive(Debug)]
pub(crate) struct TextMask {
    pub spans: Vec<MaskSpan>,
    pub bounds: Option<Rect>,
    pub caret: (i64, i64, i64),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    text: String,
    style: TextOptions,
    at: (i64, i64),
    clip: Rect,
}
pub(crate) struct TextRenderer {
    fonts: FontSystem,
    cache: SwashCache,
    last: Option<(Key, Arc<TextMask>)>,
}
impl std::fmt::Debug for TextRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextRenderer")
            .field("cached", &self.last.is_some())
            .finish_non_exhaustive()
    }
}
impl TextRenderer {
    pub fn bundled() -> Self {
        let mut db = cosmic_text::fontdb::Database::new();
        Self::load_bundled(&mut db);
        Self {
            fonts: FontSystem::new_with_locale_and_db("en-US".into(), db),
            cache: SwashCache::new(),
            last: None,
        }
    }
    fn load_bundled(db: &mut cosmic_text::fontdb::Database) {
        for bytes in [
            include_bytes!("../../../crates/presentation/paintcraft-ui/assets/selawik/selawk.ttf")
                .as_slice(),
            include_bytes!("../../../crates/presentation/paintcraft-ui/assets/selawik/selawkb.ttf")
                .as_slice(),
        ] {
            db.load_font_data(bytes.to_vec());
        }
        db.set_sans_serif_family("Selawik");
    }
    pub fn system() -> Self {
        let mut fonts = FontSystem::new();
        Self::load_bundled(fonts.db_mut());
        Self {
            fonts,
            cache: SwashCache::new(),
            last: None,
        }
    }
    pub fn families(&self) -> Vec<String> {
        paintcraft_platform::fonts::FontCatalog::from_database(self.fonts.db()).families
    }
    pub fn rasterize(
        &mut self,
        text: &str,
        style: &TextOptions,
        at: (i64, i64),
        clip: Rect,
    ) -> Result<Arc<TextMask>> {
        let key = Key {
            text: text.into(),
            style: style.clone(),
            at,
            clip,
        };
        if let Some((previous, mask)) = &self.last
            && previous == &key
        {
            return Ok(mask.clone());
        }
        if text.chars().count() > 4096
            || !(1..=512).contains(&style.points)
            || style.family.is_empty()
            || style.family.len() > 256
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "text size or length exceeds the desktop limit",
            ));
        }
        if self.cache.image_cache.len() > 2048 {
            self.cache = SwashCache::new();
        }
        let px = f32::from(style.points) * 96.0 / 72.0;
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(px, px * 1.3));
        buffer.set_size(
            Some((clip.x1 - at.0).clamp(1, 1_048_576) as f32),
            Some((clip.y1 - at.1).clamp(1, 1_048_576) as f32),
        );
        let mut attrs = Attrs::new()
            .family(Family::Name(&style.family))
            .weight(if style.bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            })
            .style(if style.italic {
                Style::Italic
            } else {
                Style::Normal
            });
        if style.underline {
            attrs = attrs.underline(UnderlineStyle::Single);
        }
        if style.strikeout {
            attrs = attrs.strikethrough();
        }
        let align = match style.alignment {
            TextAlignment::Left => Align::Left,
            TextAlignment::Center => Align::Center,
            TextAlignment::Right => Align::Right,
        };
        buffer.set_text(text, &attrs, Shaping::Advanced, Some(align));
        let mut pixels = BTreeMap::<(i64, i64), u8>::new();
        let mut exceeded = false;
        let mut work = 0u64;
        buffer.draw(
            &mut self.fonts,
            &mut self.cache,
            Color::rgb(255, 255, 255),
            |x, y, w, h, color| {
                work = work.saturating_add(u64::from(w) * u64::from(h));
                if work > 16_777_216 || pixels.len() >= PIXELS_MAX {
                    exceeded = true;
                    return;
                }
                let x0 = (at.0 + i64::from(x)).max(clip.x0);
                let x1 = (at.0 + i64::from(x) + i64::from(w)).min(clip.x1);
                let y0 = (at.1 + i64::from(y)).max(clip.y0);
                let y1 = (at.1 + i64::from(y) + i64::from(h)).min(clip.y1);
                for y in y0..y1 {
                    for x in x0..x1 {
                        if pixels.len() >= PIXELS_MAX {
                            exceeded = true;
                            return;
                        }
                        if color.a() > 0 {
                            let a = pixels.entry((y, x)).or_default();
                            *a = (u16::from(color.a())
                                + (u16::from(*a) * (255 - u16::from(color.a())) + 127) / 255)
                                as u8;
                        }
                    }
                }
            },
        );
        if exceeded {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "text raster work budget exceeded",
            ));
        }
        let mut spans: Vec<MaskSpan> = Vec::new();
        for ((y, x), coverage) in pixels {
            if let Some(last) = spans.last_mut()
                && last.y == y
                && last.x1 + 1 == x
                && last.coverage == coverage
            {
                last.x1 = x;
                continue;
            }
            spans.push(MaskSpan {
                y,
                x0: x,
                x1: x,
                coverage,
            });
        }
        if spans.len() > 262_144 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "text coverage span budget exceeded",
            ));
        }
        let mut bounds: Option<Rect> = None;
        let mut caret = (at.0, at.1, px.ceil() as i64);
        for run in buffer.layout_runs() {
            let left = run.glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
            let right = run.glyphs.iter().map(|g| g.x + g.w).fold(0.0, f32::max);
            if left.is_finite()
                && let Some(r) = Rect::new(
                    at.0 + left.floor() as i64,
                    at.1 + run.line_top.floor() as i64,
                    (right.ceil() - left.floor()).max(1.0) as i64,
                    run.line_height.ceil().max(1.0) as i64,
                )
            {
                bounds = Some(bounds.map_or(r, |b| Rect {
                    x0: b.x0.min(r.x0),
                    y0: b.y0.min(r.y0),
                    x1: b.x1.max(r.x1),
                    y1: b.y1.max(r.y1),
                }));
            }
            caret = (
                at.0 + right.ceil() as i64,
                at.1 + run.line_top.floor() as i64,
                run.line_height.ceil() as i64,
            );
        }
        let mask = Arc::new(TextMask {
            spans,
            bounds,
            caret,
        });
        self.last = Some((key, mask.clone()));
        Ok(mask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selawik_masks_have_grayscale_edges_real_styles_alignment_and_a_reused_cache() {
        let mut renderer = TextRenderer::bundled();
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
        let mut renderer = TextRenderer::system();
        let families = renderer.families();
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
