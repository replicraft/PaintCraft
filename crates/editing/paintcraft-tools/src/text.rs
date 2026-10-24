//! Portable document text shaping and grayscale Swash coverage; hosts supply fonts.
use cosmic_text::{
    Align, Attrs, Buffer, Color, Family, Metrics, Shaping, Style, SwashCache, UnderlineStyle,
    Weight,
};
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Result,
    text::{MaskSpan, TextAlignment, TextOptions},
};
use std::{collections::BTreeMap, sync::Arc};
const PIXELS_MAX: usize = 4_194_304;
pub use paintcraft_core::types::text::TextMask;
#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    text: String,
    style: TextOptions,
    at: (i64, i64),
    clip: Rect,
}
pub struct TextRenderer {
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
pub use cosmic_text::{FontSystem, fontdb::Database};
impl TextRenderer {
    pub fn new(fonts: FontSystem) -> Self {
        Self {
            fonts,
            cache: SwashCache::new(),
            last: None,
        }
    }
    pub fn database(&self) -> &Database {
        self.fonts.db()
    }
    /// Read only an already prepared mask; never shape or allocate a new key.
    pub fn cached(
        &self,
        text: &str,
        style: &TextOptions,
        at: (i64, i64),
        clip: Rect,
    ) -> Option<Arc<TextMask>> {
        let (previous, mask) = self.last.as_ref()?;
        (previous.text == text
            && previous.style == *style
            && previous.at == at
            && previous.clip == clip)
            .then(|| mask.clone())
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
                "text size or length exceeds the document limit",
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
