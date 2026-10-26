//! Fixed qualified linear spaces, independent of RGB channel names.
use super::*;
pub(super) fn to_rec709(space: LinearSpace, rgb: [f64; 3]) -> [f64; 3] {
    if space == LinearSpace::Rec709D65 {
        return rgb;
    }
    // Radiance nominal CRT (.64,.33)/(.29,.60)/(.15,.06), equal-energy
    // white -> Bradford adaptation to D65 -> linear Rec.709. Independent
    // 100-digit construction and source references live with the fixtures.
    let matrix = [
        [
            1.1644749390405187,
            -0.156546617001577,
            -0.007928322038941782,
        ],
        [
            0.007598768784493493,
            0.99301519592687,
            -0.0006139647113635745,
        ],
        [
            0.0035539147329003914,
            0.018216922364772677,
            0.978229162902327,
        ],
    ];
    matrix.map(|row| row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2])
}
pub(super) fn encode(value: f64) -> u8 {
    let encoded = if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0 + 0.5).floor() as u8
}
pub(super) fn background(value: [u8; 3]) -> [f64; 3] {
    value.map(|channel| {
        let v = f64::from(channel) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    })
}
