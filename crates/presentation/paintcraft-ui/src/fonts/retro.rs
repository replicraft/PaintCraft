//! Explicit opt-in interface typography; document fonts remain independent.
const DATA_NAME: &str = "PaintCraft W95FA";
const FAMILY_NAME: &str = "PaintCraft Retro";
static W95FA: &[u8] = include_bytes!("../../../../../assets/fonts/w95fa/W95F.otf");

/// The opt-in Retro interface family, including the current locale's fallback.
/// Register through [`super::install_for_locale`] before using this family.
#[must_use]
pub fn retro_family() -> egui::FontFamily {
    egui::FontFamily::Name(FAMILY_NAME.into())
}

pub(super) fn register(fonts: &mut egui::FontDefinitions) {
    fonts
        .font_data
        .insert(DATA_NAME.into(), egui::FontData::from_static(W95FA).into());
    let mut fallback = fonts.families[&egui::FontFamily::Proportional].clone();
    fallback.insert(0, DATA_NAME.into());
    fonts.families.insert(retro_family(), fallback);
}

#[cfg(test)]
#[path = "retro/tests.rs"]
mod tests;
