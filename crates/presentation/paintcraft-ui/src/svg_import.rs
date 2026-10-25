//! Shared SVG conversion controls. Hosts retain source bytes and private output.
use crate::{i18n::import_loss_description, t};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fit {
    #[default]
    Contain,
    Stretch,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub width: u32,
    pub height: u32,
    pub dpi: f32,
    pub fit: Fit,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            width: 512,
            height: 512,
            dpi: 96.0,
            fit: Fit::Contain,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Response {
    Pending,
    Preview(Config),
    Import,
    Cancel,
}
/// A preview and the exact private output it represents share one configuration.
/// Changing controls clears consent and prevents publication of the old output.
pub struct State<P> {
    pub config: Config,
    pub accepted: bool,
    prepared: Option<(Config, P)>,
    texture: Option<egui::TextureHandle>,
    losses: Vec<String>,
    shown: Option<u64>,
    error: Option<String>,
}
impl<P> Default for State<P> {
    fn default() -> Self {
        Self {
            config: Config::default(),
            accepted: false,
            prepared: None,
            texture: None,
            losses: Vec::new(),
            shown: None,
            error: None,
        }
    }
}
impl<P> std::fmt::Debug for State<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvgImportState")
            .field("config", &self.config)
            .field("ready", &self.prepared.is_some())
            .finish_non_exhaustive()
    }
}
impl<P> State<P> {
    fn clear(&mut self) {
        self.prepared = None;
        self.texture = None;
        self.losses.clear();
        self.accepted = false;
        self.shown = None;
        self.error = None;
    }
    /// Frame data must sample the selected full-size output, at most 256 per side.
    pub fn install(
        &mut self,
        ctx: &egui::Context,
        config: Config,
        prepared: P,
        size: [u32; 2],
        rgba: &[u8],
        losses: Vec<String>,
    ) -> bool {
        self.clear();
        if config != self.config
            || size.contains(&0)
            || size.iter().any(|v| *v > 256)
            || rgba.len() as u64 != u64::from(size[0]) * u64::from(size[1]) * 4
            || losses.is_empty()
            || losses
                .iter()
                .any(|loss| import_loss_description(loss).is_none())
        {
            return false;
        }
        self.texture = Some(ctx.load_texture(
            "svg-import-preview",
            egui::ColorImage::from_rgba_unmultiplied([size[0] as usize, size[1] as usize], rgba),
            egui::TextureOptions::NEAREST,
        ));
        self.prepared = Some((config, prepared));
        self.losses = losses;
        ctx.request_repaint();
        true
    }
    pub fn fail(&mut self, error: String) {
        self.clear();
        self.error = Some(error);
    }
    fn ready(&self, ctx: &egui::Context) -> bool {
        self.accepted
            && self
                .prepared
                .as_ref()
                .is_some_and(|(key, _)| *key == self.config)
            && self.texture.is_some()
            && self
                .shown
                .is_some_and(|frame| frame < ctx.cumulative_frame_nr())
    }
    pub fn take(&mut self, ctx: &egui::Context) -> Option<P> {
        if !self.ready(ctx) {
            return None;
        }
        let prepared = self.prepared.take().map(|(_, value)| value);
        self.clear();
        prepared
    }
    pub fn dialog(&mut self, ctx: &egui::Context, name: &str) -> Response {
        let mut response = Response::Pending;
        egui::Modal::new(egui::Id::unique("svg-import-options")).show(ctx, |ui| {
            ui.set_width(460.0_f32.min((ctx.viewport_rect().width() - 48.0).max(220.0)));
            ui.heading(t!(ctx, "Rasterize SVG"));
            ui.label(name);
            egui::ScrollArea::vertical().id_salt("svg-import-content")
                .max_height((ctx.viewport_rect().height() - 180.0).max(100.0))
                .show(ui, |ui| {
                    let before = self.config;
                    ui.horizontal_wrapped(|ui| {
                        number(ui, "Width", &mut self.config.width, 1..=16384);
                        number(ui, "Height", &mut self.config.height, 1..=16384);
                    });
                    ui.horizontal_wrapped(|ui| {
                        let label = ui.label(t!(ctx, "DPI"));
                        ui.add(egui::DragValue::new(&mut self.config.dpi).range(1.0..=2400.0))
                            .labelled_by(label.id);
                    });
                    egui::ComboBox::from_label(t!(ctx, "Fitting"))
                        .selected_text(t!(ctx, fit_label(self.config.fit)))
                        .show_ui(ui, |ui| {
                            for fit in [Fit::Contain, Fit::Stretch] {
                                ui.selectable_value(&mut self.config.fit, fit, t!(ctx, fit_label(fit)));
                            }
                        });
                    ui.label(t!(ctx, "Output is an sRGB picture with transparency. Text uses bundled fonts; external resources are disabled."));
                    ui.label(t!(ctx, "Choose up to 4,194,304 output pixels, then update the preview. Changing options requires a new preview and consent."));
                    if before != self.config || self.prepared.as_ref().is_some_and(|(key, _)| *key != self.config) {
                        self.clear();
                    }
                    if ui.button(t!(ctx, "Update preview")).clicked() {
                        self.clear();
                        response = Response::Preview(self.config);
                    }
                    if let Some(error) = &self.error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                    if let Some(texture) = &self.texture {
                        ui.label(t!(ctx, "Import preview"));
                        let size = texture.size_vec2();
                        let scale = (180.0 / size.x.max(size.y)).min(1.0);
                        let (rect, _) = ui.allocate_exact_size(size * scale, egui::Sense::hover());
                        for y in 0..(rect.height() / 8.0).ceil() as usize {
                            for x in 0..(rect.width() / 8.0).ceil() as usize {
                                let cell = egui::Rect::from_min_size(rect.min + egui::vec2(x as f32 * 8.0, y as f32 * 8.0), egui::vec2(8.0, 8.0)).intersect(rect);
                                ui.painter().rect_filled(cell, 0.0, egui::Color32::from_gray(if (x+y)%2 == 0 { 160 } else { 210 }));
                            }
                        }
                        ui.painter().image(texture.id(), rect, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                        if self.shown.is_none() && ui.is_rect_visible(rect) {
                            self.shown = Some(ctx.cumulative_frame_nr());
                            ctx.request_repaint();
                        }
                        for loss in &self.losses {
                            if let Some(text) = import_loss_description(loss) { ui.label(t!(ctx, text)); }
                        }
                        ui.checkbox(&mut self.accepted, t!(ctx, "I accept these import changes."));
                    }
                });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(t!(ctx, "Cancel")).clicked() { response = Response::Cancel; }
                if ui.add_enabled(self.ready(ctx), egui::Button::new(t!(ctx, "Import image"))).clicked() {
                    response = Response::Import;
                }
            });
        });
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            response = Response::Cancel;
        }
        response
    }
}
fn number(ui: &mut egui::Ui, source: &str, value: &mut u32, range: std::ops::RangeInclusive<u32>) {
    let text = t!(ui.ctx(), source);
    let label = ui.label(&text);
    ui.add(egui::DragValue::new(value).range(range))
        .labelled_by(label.id)
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::SpinButton, ui.is_enabled(), &text));
}
fn fit_label(fit: Fit) -> &'static str {
    match fit {
        Fit::Contain => "Preserve aspect ratio",
        Fit::Stretch => "Stretch to size",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_requires_displayed_preview_consent_and_unchanged_options() {
        let ctx = egui::Context::default();
        let mut state = State::default();
        let config = state.config;
        let install = |state: &mut State<u8>| {
            assert!(state.install(
                &ctx,
                config,
                9,
                [1, 1],
                &[10, 20, 30, 255],
                vec!["svg-rasterized".into()]
            ));
        };
        install(&mut state);
        state.accepted = true;
        assert!(
            state.take(&ctx).is_none(),
            "an undisplayed preview cannot be accepted"
        );
        for _ in 0..3 {
            let mut output = ctx.run_ui(Default::default(), |ui| {
                state.dialog(ui.ctx(), "source.svg");
            });
            output.textures_delta.clear();
        }
        assert!(state.take(&ctx).is_some());
        install(&mut state);
        for _ in 0..3 {
            let mut output = ctx.run_ui(Default::default(), |ui| {
                state.dialog(ui.ctx(), "source.svg");
            });
            output.textures_delta.clear();
        }
        assert!(state.take(&ctx).is_none(), "each new preview needs consent");
        state.accepted = true;
        state.config.dpi = 120.0;
        assert!(
            state.take(&ctx).is_none(),
            "changed geometry must not publish old output"
        );
        for _ in 0..3 {
            let mut output = ctx.run_ui(Default::default(), |ui| {
                state.dialog(ui.ctx(), "source.svg");
            });
            output.textures_delta.clear();
        }
        assert!(!state.accepted);
        assert!(state.texture.is_none());
        assert!(!state.install(
            &ctx,
            state.config,
            9,
            [1, 1],
            &[0; 4],
            vec!["unknown-loss".into()]
        ));
    }
    #[test]
    fn conversion_controls_are_translated_in_every_language() {
        for language in crate::i18n::Language::ALL
            .into_iter()
            .filter(|lang| *lang != crate::i18n::Language::English)
        {
            for source in [
                "Rasterize SVG",
                "Fitting",
                "Preserve aspect ratio",
                "Stretch to size",
                "Update preview",
                "Import preview",
                "I accept these import changes.",
                "Import image",
                "Output is an sRGB picture with transparency. Text uses bundled fonts; external resources are disabled.",
                "Choose up to 4,194,304 output pixels, then update the preview. Changing options requires a new preview and consent.",
            ] {
                assert_ne!(
                    crate::i18n::translate(language, source),
                    source,
                    "{}: {source}",
                    language.id()
                );
            }
        }
    }
}
