//! Typed asset DTOs isolate the selected codec from frontend contracts.
use super::*;
use ag_psd::ase::{Ase, AseColor, AseColorType as K, AseColorValue as V, AseEntry, AseGroup};
fn swatch(value: AseColor) -> Swatch {
    let (color, kind) = match value.color {
        V::Rgb { r, g, b, type_ } => (Color::Rgb([r, g, b]), type_),
        V::Cmyk { c, m, y, k, type_ } => (Color::Cmyk([c, m, y, k]), type_),
        V::Gray { k, type_ } => (Color::Gray(k), type_),
        V::Lab { l, a, b, type_ } => (Color::Lab([l, a, b]), type_),
    };
    Swatch {
        name: value.name,
        color,
        kind: match kind {
            K::Global => Kind::Global,
            K::Spot => Kind::Spot,
            K::Normal => Kind::Normal,
        },
    }
}
pub(super) fn from_backend(value: Ase) -> Vec<Entry> {
    value
        .colors
        .into_iter()
        .map(|entry| match entry {
            AseEntry::Color(color) => Entry::Swatch(swatch(color)),
            AseEntry::Group(group) => Entry::Group {
                name: group.name,
                swatches: group.colors.into_iter().map(swatch).collect(),
            },
        })
        .collect()
}
fn color(value: &Swatch) -> AseColor {
    let type_ = match value.kind {
        Kind::Global => K::Global,
        Kind::Spot => K::Spot,
        Kind::Normal => K::Normal,
    };
    AseColor {
        name: value.name.clone(),
        color: match value.color {
            Color::Rgb([r, g, b]) => V::Rgb { r, g, b, type_ },
            Color::Cmyk([c, m, y, k]) => V::Cmyk { c, m, y, k, type_ },
            Color::Gray(k) => V::Gray { k, type_ },
            Color::Lab([l, a, b]) => V::Lab { l, a, b, type_ },
        },
    }
}
pub(super) fn to_backend(value: &Palette) -> Ase {
    Ase {
        colors: value
            .entries
            .iter()
            .map(|entry| match entry {
                Entry::Swatch(swatch) => AseEntry::Color(color(swatch)),
                Entry::Group { name, swatches } => AseEntry::Group(AseGroup {
                    name: name.clone(),
                    colors: swatches.iter().map(color).collect(),
                }),
            })
            .collect(),
    }
}
