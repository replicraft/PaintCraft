//! Desktop appearance integration.

use super::*;

impl App {
    /// Load saved settings from `dir`. With no saved Classic variant the first-visit
    /// picker is shown, with the default (v11) selected.
    pub fn with_settings(mut self, dir: PathBuf) -> App {
        let (settings, loaded) = Settings::load(&dir);
        if let Loaded::Unusable { reason, moved_to } = loaded {
            let kept = moved_to.map_or_else(String::new, |p| format!(" (kept as {})", p.display()));
            self.editor.set_status(format!(
                "Settings were unusable ({reason}){kept}; using defaults"
            ));
        }
        self.picking = (settings.experience == Experience::Classic
            && settings.classic_variant.is_none())
        .then_some(ClassicVariant::DEFAULT);
        self.settings = settings;
        self.settings_dir = Some(dir);
        self
    }

    /// The Classic variant in effect (the default until one is chosen).
    pub fn variant(&self) -> ClassicVariant {
        self.settings
            .classic_variant
            .unwrap_or(ClassicVariant::DEFAULT)
    }

    pub fn experience(&self) -> Experience {
        self.settings.experience
    }

    pub(super) fn choose_experience(&mut self, experience: Experience) {
        if self.settings.experience == experience {
            return;
        }
        self.editor.cancel_gesture();
        self.editor.commit_text();
        self.settings.experience = experience;
        self.picking = (experience == Experience::Classic
            && self.settings.classic_variant.is_none())
        .then_some(ClassicVariant::DEFAULT);
        self.persist_settings();
    }

    fn persist_settings(&mut self) {
        if let Some(dir) = &self.settings_dir
            && let Err(e) = self.settings.save(dir)
        {
            self.editor
                .set_status(format!("Could not save the interface choice: {e}"));
        }
    }

    pub fn picker_open(&self) -> bool {
        self.picking.is_some()
    }

    pub(super) fn choose_variant(&mut self, v: ClassicVariant) {
        self.settings.classic_variant = Some(v);
        if let Some(dir) = &self.settings_dir
            && let Err(e) = self.settings.save(dir)
        {
            self.editor
                .set_status(format!("Could not save the interface choice: {e}"));
        }
        self.picking = None;
    }

    /// Presentation presets. These are look-and-feel starting points, not captured
    /// reproductions of the original applications.
    pub(super) fn apply_style(&mut self, ctx: &egui::Context) {
        let v = self.variant();
        let experience = self.experience();
        if self.applied_variant == Some((experience, v)) {
            return;
        }
        if self.applied_variant.is_none() {
            paintcraft_ui::fonts::install(ctx);
        }
        let style = egui::Style {
            visuals: egui::Visuals::light(),
            ..Default::default()
        };
        ctx.set_style_of(egui::Theme::Light, style);
        match experience {
            Experience::Classic => paintcraft_classic::apply_style(ctx, v),
            Experience::Studio => paintcraft_studio::apply_style(ctx),
        }
        self.applied_variant = Some((experience, v));
    }

    /// First-visit picker, also reachable later from the toolbar.
    pub(super) fn picker_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(mut current) = self.picking else {
            return;
        };
        let ctx = ui.ctx().clone();
        let mut done = false;
        egui::Window::new("Choose your Paint")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(&ctx, |ui| {
                ui.label("Pick the look you know. You can change it later from Interface.");
                ui.horizontal(|ui| {
                    for v in ClassicVariant::ALL {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(150.0, 110.0), Sense::click());
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::RadioButton,
                                true,
                                current == v,
                                v.name(),
                            )
                        });
                        if response.clicked() {
                            current = v;
                        }
                        draw_variant_thumbnail(ui.painter(), rect, v, current == v);
                    }
                });
                ui.horizontal(|ui| {
                    for v in ClassicVariant::ALL {
                        ui.add_sized([150.0, 18.0], egui::Label::new(v.name()));
                    }
                });
                if ui.button("Continue").clicked() {
                    done = true;
                }
            });
        if done {
            self.choose_variant(current);
        } else {
            self.picking = Some(current);
        }
    }
}

/// A small schematic of each variant's window, so the choice is visual.
fn draw_variant_thumbnail(p: &egui::Painter, rect: Rect, v: ClassicVariant, selected: bool) {
    let (bar, ribbon, canvas) = match v {
        ClassicVariant::V7 => (
            Color32::from_rgb(120, 160, 215),
            Color32::from_rgb(214, 224, 240),
            Color32::WHITE,
        ),
        ClassicVariant::V10 => (
            Color32::from_rgb(255, 255, 255),
            Color32::from_rgb(235, 235, 235),
            Color32::WHITE,
        ),
        ClassicVariant::V11 => (
            Color32::from_rgb(243, 243, 243),
            Color32::from_rgb(250, 250, 250),
            Color32::WHITE,
        ),
    };
    let round = if v == ClassicVariant::V11 { 8.0 } else { 0.0 };
    let stroke = if selected {
        egui::Stroke::new(3.0, Color32::from_rgb(0, 95, 184))
    } else {
        egui::Stroke::new(1.0, Color32::GRAY)
    };
    p.rect_filled(rect, round, Color32::from_gray(220));
    p.rect_filled(
        Rect::from_min_size(rect.min, Vec2::new(rect.width(), 14.0)),
        round,
        bar,
    );
    p.rect_filled(
        Rect::from_min_size(
            rect.min + Vec2::new(0.0, 14.0),
            Vec2::new(rect.width(), 28.0),
        ),
        0.0,
        ribbon,
    );
    for i in 0..6 {
        let x = rect.min.x + 8.0 + i as f32 * 14.0;
        p.rect_filled(
            Rect::from_min_size(Pos2::new(x, rect.min.y + 20.0), Vec2::splat(10.0)),
            if v == ClassicVariant::V11 { 5.0 } else { 0.0 },
            Color32::from_gray(150),
        );
    }
    p.rect_filled(
        Rect::from_min_max(
            rect.min + Vec2::new(8.0, 50.0),
            rect.max - Vec2::new(8.0, 8.0),
        ),
        0.0,
        canvas,
    );
    p.rect_stroke(rect, round, stroke, egui::StrokeKind::Inside);
}
