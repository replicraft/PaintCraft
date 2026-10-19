//! Bundled interface typography, independent of the host's installed fonts.

/// Use Selawik for proportional UI text, retaining egui's fallback glyphs and
/// monospace family. Install once when initializing a frontend's context.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Selawik".into(),
        egui::FontData::from_static(include_bytes!("../assets/selawik/selawk.ttf")).into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Selawik".into());
    ctx.set_fonts(fonts);
}
