//! Stream bitmap text into an admitted sink without retaining a geometry vector.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};

/// Visit clipped spans in the same glyph/row/run order as [`layout`].
/// The sink owns allocation policy and must discard partial output on error.
/// This is the deterministic bitmap font, not native font shaping/rasterization.
pub fn visit_layout(
    text: &str,
    at: (i64, i64),
    scale: i64,
    clip: &Rect,
    check: &dyn Fn() -> Result<()>,
    mut emit: impl FnMut(Span) -> Result<()>,
) -> Result<()> {
    check()?;
    if !(1..=SCALE_MAX).contains(&scale) {
        return Err(Error::new(ErrorCode::InputSchema, "invalid text scale"));
    }
    let scale = i128::from(scale);
    let (mut line, mut col) = (0i128, 0i128);
    for (index, c) in text.chars().enumerate() {
        check()?;
        if index >= TEXT_CHARS_MAX {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "text exceeds character limit",
            ));
        }
        if c == '\n' {
            line += 1;
            col = 0;
            continue;
        }
        // Validate even offscreen glyphs, retaining the original failure policy.
        let rows = glyph(c).ok_or_else(|| {
            Error::new(
                ErrorCode::CapabilityUnsupported,
                format!(
                    "character U+{:04X} has no glyph in the built-in font",
                    c as u32
                ),
            )
        })?;
        let gx = i128::from(at.0) + col * i128::from(CELL) * scale;
        let gy = i128::from(at.1) + line * i128::from(CELL) * scale;
        col += 1;
        if clip.x0 >= clip.x1
            || clip.y0 >= clip.y1
            || gx + i128::from(CELL) * scale <= i128::from(clip.x0)
            || gx >= i128::from(clip.x1)
            || gy + i128::from(CELL) * scale <= i128::from(clip.y0)
            || gy >= i128::from(clip.y1)
        {
            continue;
        }
        for (r, bits) in rows.iter().enumerate() {
            check()?;
            let y0 = gy + r as i128 * scale;
            let first = y0.max(i128::from(clip.y0));
            let end = (y0 + scale).min(i128::from(clip.y1));
            if first >= end {
                continue;
            }
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
                let x0 = (gx + i128::from(start) * scale).max(i128::from(clip.x0));
                let x1 = (gx + i128::from(x) * scale - 1).min(i128::from(clip.x1) - 1);
                if x0 <= x1 {
                    for y in first..end {
                        check()?;
                        emit(Span {
                            y: y as i64,
                            x0: x0 as i64,
                            x1: x1 as i64,
                        })?;
                    }
                }
            }
        }
    }
    check()
}
