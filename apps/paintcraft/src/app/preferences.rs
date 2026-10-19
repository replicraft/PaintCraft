//! Application settings with reversible theme preview and explicit adoption.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Appearance,
    Canvas,
    Text,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SettingsDraft {
    pub values: Settings,
    page: Page,
}

impl App {
    pub(super) fn open_settings(&mut self, ctx: &egui::Context) {
        self.resolve_tab_input(ctx);
        self.confirm = Some(Confirm::Settings(SettingsDraft {
            values: self.settings.clone(),
            page: Page::Appearance,
        }));
    }

    pub(super) fn settings_dialog(&mut self, ctx: &egui::Context, mut draft: SettingsDraft) {
        let mut apply = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::unique("application-settings")).show(ctx, |ui| {
            ui.set_width(640.0);
            ui.set_height(365.0);
            ui.heading("Settings");
            ui.add_space(10.0);
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(egui::vec2(150.0,260.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                    for (page,label) in [(Page::Appearance,"Appearance"),(Page::Canvas,"Canvas"),(Page::Text,"Text preferences")] {
                        ui.selectable_value(&mut draft.page,page,label);
                    }
                });
                ui.separator();
                ui.vertical(|ui| match draft.page {
                    Page::Appearance => appearance(ui,&mut draft.values),
                    Page::Canvas => canvas(ui,&mut draft.values.canvas),
                    Page::Text => {
                        ui.strong("Text editing");
                        ui.label(format!("{} available font families",self.fonts.families.len()));
                        ui.checkbox(&mut draft.values.text.editable_until_save,"Keep text editable until save (prototype)");
                        ui.weak("Prototype preference only. Editable text objects and save-time rasterization are not implemented yet.");
                    }
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                apply = ui.button("Apply").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if ctx.current_pass_index() != 0 {
            return;
        }
        if cancel || response.should_close() {
            self.confirm = None;
            ctx.request_repaint();
        } else if apply {
            let experience = draft.values.experience;
            draft.values.experience = self.experience();
            self.settings = draft.values;
            self.confirm = None;
            self.choose_experience(experience);
            self.persist_settings();
            ctx.request_repaint();
        } else {
            self.confirm = Some(Confirm::Settings(draft));
        }
    }
}

fn appearance(ui: &mut egui::Ui, values: &mut Settings) {
    ui.strong("App theme");
    for (value, label) in [
        (settings::Appearance::System, "Use system setting"),
        (settings::Appearance::Light, "Light"),
        (settings::Appearance::Dark, "Dark"),
    ] {
        ui.radio_value(&mut values.appearance, value, label);
    }
    ui.add_space(18.0);
    ui.strong("Interface");
    ui.weak("Each interface has a separate workspace. Save or discard changes before switching.");
    for (value, label) in [
        (Experience::Classic, "Classic interface"),
        (Experience::Studio, "Studio interface"),
    ] {
        ui.radio_value(&mut values.experience, value, label);
    }
    if values.experience == Experience::Classic {
        ui.add_space(10.0);
        let mut variant = values.classic_variant.unwrap_or(ClassicVariant::DEFAULT);
        for v in ClassicVariant::ALL {
            if ui.radio_value(&mut variant, v, v.name()).changed() {
                values.classic_variant = Some(variant);
            }
        }
    }
}

fn canvas(ui: &mut egui::Ui, values: &mut settings::CanvasDefaults) {
    ui.strong("New Classic canvas");
    for (label, value) in [
        ("Default width", &mut values.configured.0),
        ("Default height", &mut values.configured.1),
    ] {
        ui.horizontal(|ui| {
            let caption = ui.label(label);
            ui.add(
                egui::DragValue::new(value)
                    .range(1..=settings::SIZE_MAX)
                    .suffix(" px"),
            )
            .labelled_by(caption.id);
        });
    }
    ui.radio_value(&mut values.remember_last, true, "Remember last canvas size");
    ui.radio_value(
        &mut values.remember_last,
        false,
        "Use configured default size",
    );
    if let Some((w, h)) = values.last {
        ui.weak(format!("Last canvas: {w} × {h} pixels"));
    }
}
