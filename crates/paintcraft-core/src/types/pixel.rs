//! Straight (non-premultiplied) RGBA8 and the integer baseline arithmetic of
//! policy `paintcraft.core/default-v1`.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba([0, 0, 0, 0]);

    pub fn a(self) -> u8 {
        self.0[3]
    }

    /// Parses `#RRGGBBAA` (either case).
    pub fn parse(text: &str) -> Option<Rgba> {
        let b = text.as_bytes();
        if b.len() != 9 || b[0] != b'#' || !b[1..].iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        let v = |i: usize| u8::from_str_radix(&text[i..i + 2], 16).ok();
        Some(Rgba([v(1)?, v(3)?, v(5)?, v(7)?]))
    }

    /// Uppercase `#RRGGBBAA`.
    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.0;
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }

    /// Layer opacity: `alpha' = floor((alpha * opacity + 127) / 255)`; RGB untouched.
    pub fn with_opacity(self, opacity: u8) -> Rgba {
        let [r, g, b, a] = self.0;
        let a = ((u32::from(a) * u32::from(opacity) + 127) / 255) as u8;
        Rgba([r, g, b, a])
    }

    /// Straight RGBA8 source-over, rounding half up. A zero-alpha source leaves
    /// the destination untouched, preserving its hidden RGB.
    pub fn source_over(self, dst: Rgba) -> Rgba {
        let (a, b) = (u32::from(self.0[3]), u32::from(dst.0[3]));
        if a == 0 {
            return dst;
        }
        let total = a * 255 + b * (255 - a);
        let mut out = [0u8; 4];
        for (o, (&s, &d)) in out.iter_mut().zip(self.0.iter().zip(dst.0.iter())).take(3) {
            let (s, d) = (u32::from(s), u32::from(d));
            *o = ((s * a * 255 + d * b * (255 - a) + total / 2) / total) as u8;
        }
        out[3] = ((total + 127) / 255) as u8;
        Rgba(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_format_round_trip() {
        assert_eq!(Rgba::parse("#ff8800Ab"), Some(Rgba([255, 136, 0, 171])));
        assert_eq!(Rgba([255, 136, 0, 171]).to_hex(), "#FF8800AB");
        for bad in [
            "",
            "#FFF",
            "FF8800AB0",
            "#GG8800AB",
            "#ff8800ab0",
            "#ff8800a",
        ] {
            assert_eq!(Rgba::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn source_over_matches_hand_computed_values() {
        let over = |s: [u8; 4], d: [u8; 4]| Rgba(s).source_over(Rgba(d)).0;
        assert_eq!(over([255, 0, 0, 128], [0, 0, 255, 255]), [128, 0, 127, 255]);
        assert_eq!(over([0, 255, 0, 64], [0, 0, 255, 255]), [0, 64, 191, 255]);
        assert_eq!(over([0, 0, 255, 128], [0, 0, 255, 128]), [0, 0, 255, 192]);
        assert_eq!(over([9, 9, 9, 255], [1, 2, 3, 4]), [9, 9, 9, 255]);
    }

    #[test]
    fn zero_alpha_source_preserves_hidden_rgb() {
        assert_eq!(
            Rgba([1, 2, 3, 0]).source_over(Rgba([7, 8, 9, 0])).0,
            [7, 8, 9, 0]
        );
    }

    #[test]
    fn opacity_formula() {
        let p = Rgba([1, 2, 3, 128]);
        assert_eq!(p.with_opacity(255).0, [1, 2, 3, 128]);
        assert_eq!(p.with_opacity(128).0, [1, 2, 3, 64]);
        assert_eq!(p.with_opacity(1).0, [1, 2, 3, 1]);
        assert_eq!(Rgba([1, 2, 3, 1]).with_opacity(127).0, [1, 2, 3, 0]);
        assert!((0..=255u8).all(|a| Rgba([0, 0, 0, a]).with_opacity(255).a() == a));
    }
}
