//! Application settings with staged choices and explicit adoption.
use super::*;
use paintcraft_classic::settings::{Page, Response, card};
use paintcraft_ui::i18n::picker;
use paintcraft_ui::t;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SettingsDraft {
    pub values: Settings,
    page: Page,
    font_search: String,
}

impl App {
    pub(super) fn open_settings(&mut self, ctx: &egui::Context) {
        self.resolve_tab_input(ctx);
        self.confirm = Some(Confirm::Settings(SettingsDraft {
            values: self.settings.clone(),
            page: Page::Appearance,
            font_search: String::new(),
        }));
    }

    pub(super) fn settings_dialog(&mut self, ctx: &egui::Context, mut draft: SettingsDraft) {
        let classic = self.experience() == Experience::Classic;
        let mut page = draft.page;
        let response = paintcraft_classic::settings::show(
            ctx,
            classic,
            &mut page,
            |ui, page, width, height| {
                content(
                    ui,
                    &mut draft,
                    &self.fonts.families,
                    page,
                    width,
                    height,
                    classic,
                );
            },
        );
        draft.page = page;
        match response {
            Response::Cancel => {
                self.confirm = None;
                ctx.request_repaint();
            }
            Response::Apply => {
                let experience = draft.values.experience;
                draft.values.experience = self.experience();
                self.settings = draft.values;
                self.confirm = None;
                self.choose_experience(experience);
                self.persist_settings();
                ctx.request_repaint();
            }
            Response::Pending => self.confirm = Some(Confirm::Settings(draft)),
        }
    }
}

fn content(
    ui: &mut egui::Ui,
    draft: &mut SettingsDraft,
    fonts: &[String],
    page: Page,
    width: f32,
    height: f32,
    classic: bool,
) {
    ui.allocate_ui_with_layout(egui::vec2(width, height), egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(width);
        if classic {
            paintcraft_classic::form_control_style(ui);
        }
        egui::ScrollArea::vertical().id_salt(("settings-page", page))
            .max_height(height).auto_shrink([false, false]).show(ui, |ui| {
                match page {
                    Page::Appearance => appearance(ui, &mut draft.values),
                    Page::About => card(ui, "About", |ui| {
                        paintcraft_classic::settings::about(ui);
                    }),
                    Page::Interface => card(ui, "Canvas navigation", |ui| {
                        ui.checkbox(&mut draft.values.studio_auto_scroll, t!(ui.ctx(), "Auto-scroll"));
                        ui.label(t!(ui.ctx(), "Scroll automatically while dragging near the edge of the canvas viewport."));
                        ui.checkbox(&mut draft.values.studio_overscroll, t!(ui.ctx(), "Scroll past the edge of the image (overscroll)"));
                        ui.label(t!(ui.ctx(), "Move small images halfway offscreen and large image edges to the viewport center."));
                    }),
                    Page::Canvas => canvas(ui, &mut draft.values.canvas, classic),
                    Page::Text => card(ui, "Text editing", |ui| {
                        ui.label(t!(ui.ctx(), "Default font"));
                        paintcraft_classic::font_family(ui, &mut draft.values.text.family, fonts, &mut draft.font_search, false);
                        ui.label(t!(ui.ctx(), "{ $count } available font families", &[("count", fonts.len().to_string())]));
                        ui.checkbox(&mut draft.values.text.editable_until_save,
                            t!(ui.ctx(), "Keep text editable until save (prototype)"));
                        ui.weak(t!(ui.ctx(), "Prototype preference only. Text is rasterized when placed; editable text objects and save-time rasterization are not available yet."));
                    }),
                }
            });
    });
}

fn appearance(ui: &mut egui::Ui, values: &mut Settings) {
    card(ui, "App language", |ui| {
        picker(ui, &mut values.language);
    });
    card(ui, "App theme", |ui| {
        ui.weak(t!(ui.ctx(), "Changes take effect when you select Apply.",));
        if values.experience == Experience::Classic
            && values.classic_variant == Some(ClassicVariant::V7)
        {
            ui.label(t!(
                ui.ctx(),
                "Paint 7 uses light mode. Your preferred theme returns with other skins."
            ));
            ui.add_enabled(false, egui::RadioButton::new(true, t!(ui.ctx(), "Light")));
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for (value, label) in [
                (settings::Appearance::System, "Use system setting"),
                (settings::Appearance::Light, "Light"),
                (settings::Appearance::Dark, "Dark"),
            ] {
                ui.radio_value(&mut values.appearance, value, t!(ui.ctx(), label));
            }
        });
    });
    card(ui, "Interface", |ui| {
        ui.weak(
            "Each interface has a separate workspace. Save or discard changes before switching.",
        );
        ui.horizontal_wrapped(|ui| {
            for (value, label) in [
                (Experience::Classic, "Classic interface"),
                (Experience::Studio, "Studio interface"),
            ] {
                ui.radio_value(&mut values.experience, value, t!(ui.ctx(), label));
            }
        });
    });
    if values.experience == Experience::Classic {
        card(ui, "Classic skin", |ui| {
            let mut variant = values.classic_variant.unwrap_or(ClassicVariant::DEFAULT);
            ui.horizontal_wrapped(|ui| {
                for v in ClassicVariant::ALL {
                    if ui.radio_value(&mut variant, v, v.name()).changed() {
                        values.classic_variant = Some(variant);
                    }
                }
            });
        });
    }
}

fn canvas(ui: &mut egui::Ui, values: &mut settings::CanvasDefaults, classic: bool) {
    card(ui, "New Classic canvas", |ui| {
        ui.weak(t!(
            ui.ctx(),
            "These defaults apply to new documents, leaving the current image unchanged.",
        ));
        if classic {
            paintcraft_classic::canvas_size::dimension_fields(
                ui,
                &mut values.configured,
                settings::SIZE_MAX,
                ["Default width", "Default height"],
            );
        } else {
            for (label, value) in [
                ("Default width", &mut values.configured.0),
                ("Default height", &mut values.configured.1),
            ] {
                ui.horizontal(|ui| {
                    let caption = ui.label(t!(ui.ctx(), label));
                    ui.add(
                        egui::DragValue::new(value)
                            .range(1..=settings::SIZE_MAX)
                            .suffix(" px"),
                    )
                    .labelled_by(caption.id);
                });
            }
        }
    });
    card(ui, "Canvas size on startup", |ui| {
        ui.radio_value(
            &mut values.remember_last,
            true,
            t!(ui.ctx(), "Remember last canvas size"),
        );
        ui.radio_value(
            &mut values.remember_last,
            false,
            t!(ui.ctx(), "Use configured default size"),
        );
        if let Some((w, h)) = values.last {
            ui.weak(t!(
                ui.ctx(),
                "Last canvas: { $width } × { $height } pixels",
                &[("width", w.to_string()), ("height", h.to_string())],
            ));
        }
    });
}
