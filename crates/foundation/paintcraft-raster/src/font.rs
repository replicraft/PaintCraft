//! A built-in 8×8 bitmap font drawn at whole-number scales.
//!
//! Text is exact and deterministic: no hinting, no anti-aliasing, no platform
//! fonts. Characters outside Basic Latin and Latin-1 have no glyph and are
//! refused rather than substituted. This is the starting point for text, not a
//! claim of fidelity to any application's text tool.

use font8x8::{BASIC_FONTS, LATIN_FONTS, UnicodeFonts};
use paintcraft_core::types::Rect;

use crate::shape::Span;
mod checked;
pub use checked::visit_layout;

/// Pixels per glyph cell at scale 1.
pub const CELL: i64 = 8;
pub use paintcraft_core::types::policy::SCALE_MAX;
pub use paintcraft_core::types::policy::TEXT_CHARS_MAX;

/// Row bitmaps, least significant bit leftmost. Control characters other than
/// newline, and the DEL range, have no glyph.
pub fn glyph(c: char) -> Option<[u8; 8]> {
    if c.is_control() {
        return None;
    }
    BASIC_FONTS.get(c).or_else(|| LATIN_FONTS.get(c))
}

pub fn supported(c: char) -> bool {
    c == '\n' || glyph(c).is_some()
}

/// Spans of the text laid out from the top-left corner `at`, clipped to `clip`.
/// `Err(c)` names the first character that has no glyph.
pub fn layout(text: &str, at: (i64, i64), scale: i64, clip: &Rect) -> Result<Vec<Span>, char> {
    let mut out = Vec::new();
    let (mut line, mut col) = (0i64, 0i64);
    for c in text.chars() {
        if c == '\n' {
            line += 1;
            col = 0;
            continue;
        }
        let rows = glyph(c).ok_or(c)?;
        let (gx, gy) = (at.0 + col * CELL * scale, at.1 + line * CELL * scale);
        col += 1;
        if gx + CELL * scale <= clip.x0
            || gx >= clip.x1
            || gy + CELL * scale <= clip.y0
            || gy >= clip.y1
        {
            continue;
        }
        for (r, bits) in rows.iter().enumerate() {
            let mut x = 0;
            while x < 8 {
                if bits >> x & 1 == 0 {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < 8 && bits >> x & 1 == 1 {
                    x += 1;
                }
                let (x0, x1) = (gx + start * scale, gx + x * scale - 1);
                for dy in 0..scale {
                    let y = gy + r as i64 * scale + dy;
                    let (cx0, cx1) = (x0.max(clip.x0), x1.min(clip.x1 - 1));
                    if cx0 <= cx1 && (clip.y0..clip.y1).contains(&y) {
                        out.push(Span {
                            y,
                            x0: cx0,
                            x1: cx1,
                        });
                    }
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::render;

    fn big() -> Rect {
        Rect::new(0, 0, 100, 40).unwrap()
    }

    #[test]
    fn a_glyph_scales_by_whole_pixels() {
        let one = render(&layout("A", (0, 0), 1, &big()).unwrap(), 8, 8);
        assert_eq!(
            glyph('A'),
            Some([0x0C, 0x1E, 0x33, 0x33, 0x3F, 0x33, 0x33, 0x00])
        );
        // Bit 0 is the leftmost pixel: 0x0C = 00001100 is "..##....".
        assert_eq!(
            one,
            [
                "..##....", ".####...", "##..##..", "##..##..", "######..", "##..##..", "##..##..",
                "........"
            ]
        );
        let two = render(&layout("A", (0, 0), 2, &big()).unwrap(), 16, 16);
        for (y, row) in two.iter().enumerate() {
            let src = &one[y / 2];
            assert_eq!(
                row,
                &src.chars().flat_map(|c| [c, c]).collect::<String>(),
                "row {y}"
            );
        }
    }

    #[test]
    fn lines_columns_and_clipping_follow_the_cell_grid() {
        let spans = layout("A\nA", (3, 2), 1, &big()).unwrap();
        assert!(
            spans.iter().any(|s| s.y == 2 + 8 + 4),
            "second line starts 8 rows lower"
        );
        let both = layout("AA", (0, 0), 1, &big()).unwrap();
        assert!(
            both.iter().any(|s| s.x0 >= 8),
            "second glyph starts one cell to the right"
        );
        let clip = Rect::new(2, 2, 4, 3).unwrap();
        for s in layout("AAAA", (0, 0), 3, &clip).unwrap() {
            assert!(
                clip.contains(s.x0, s.y) && clip.contains(s.x1, s.y),
                "{s:?}"
            );
        }
        assert!(layout("A", (500, 500), 1, &big()).unwrap().is_empty());
    }

    #[test]
    fn unsupported_characters_are_named_not_substituted() {
        assert_eq!(
            layout("ab\u{1F600}c", (0, 0), 1, &big()).unwrap_err(),
            '\u{1F600}'
        );
        assert_eq!(layout("tab\there", (0, 0), 1, &big()).unwrap_err(), '\t');
        assert!(
            supported('é')
                && supported(' ')
                && supported('\n')
                && !supported('\u{0}')
                && !supported('漢')
        );
    }
}
