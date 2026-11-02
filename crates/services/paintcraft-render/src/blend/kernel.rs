//! Exact opaque reduction uses unrounded Normal numerator; no generic iterator.
use super::*;
fn multiply(s: u32, d: u32, a: u32) -> u8 {
    let normal = s * a + d * (255 - a);
    let numerator = 255 * normal - a * s * (255 - d);
    ((numerator + 32_512) / 65_025) as u8
}
fn screen(s: u32, d: u32, a: u32) -> u8 {
    let normal = s * a + d * (255 - a);
    let numerator = 255 * normal + a * d * (255 - s);
    ((numerator + 32_512) / 65_025) as u8
}
pub(super) fn pixel(kind: Kind, source: Rgba, backdrop: Rgba) -> Result<Rgba> {
    match kind {
        Kind::Normal => Ok(source.source_over(backdrop)),
        Kind::General(mode) => Ok(mode.composite(source, backdrop)),
        Kind::Opaque(mode) => {
            if backdrop.a() != 255 {
                return Err(Error::new(
                    ErrorCode::InvariantViolation,
                    "Opaque CPU blend certificate changed",
                ));
            }
            let a = u32::from(source.a());
            if a == 0 {
                return Ok(backdrop);
            }
            let s = source.0;
            let d = backdrop.0;
            let rgba = match mode {
                Mode::Multiply => [
                    multiply(u32::from(s[0]), u32::from(d[0]), a),
                    multiply(u32::from(s[1]), u32::from(d[1]), a),
                    multiply(u32::from(s[2]), u32::from(d[2]), a),
                    255,
                ],
                Mode::Screen => [
                    screen(u32::from(s[0]), u32::from(d[0]), a),
                    screen(u32::from(s[1]), u32::from(d[1]), a),
                    screen(u32::from(s[2]), u32::from(d[2]), a),
                    255,
                ],
                Mode::Normal => {
                    return Err(Error::new(
                        ErrorCode::InvariantViolation,
                        "Opaque blend selected Normal",
                    ));
                }
            };
            Ok(Rgba(rgba))
        }
    }
}

#[cfg(test)]
mod tests;
