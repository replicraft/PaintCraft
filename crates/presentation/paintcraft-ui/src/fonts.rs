//! Bundled interface typography, independent of the host's installed fonts.

pub const SELAWIK_REGULAR: &[u8] = include_bytes!("../assets/selawik/selawk.ttf");
pub const SELAWIK_BOLD: &[u8] = include_bytes!("../assets/selawik/selawkb.ttf");

/// Use Selawik for proportional UI text, retaining egui's fallback glyphs and
/// monospace family. Install once when initializing a frontend's context.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Selawik".into(),
        egui::FontData::from_static(SELAWIK_REGULAR).into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Selawik".into());
    ctx.set_fonts(fonts);
}
