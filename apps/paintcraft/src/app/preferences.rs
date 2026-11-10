//! Application settings with staged choices and explicit adoption.
use super::*;
use paintcraft_classic::settings::{Page, Response, card};
use paintcraft_ui::i18n::picker;
use paintcraft_ui::t;
#[cfg(test)]
#[path = "preferences/pen_tests.rs"]
mod pen_tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SettingsDraft {
    pub values: Settings,
    retro: paintcraft_retro::Workspace,
    page: Page,
    font_search: String,
}

impl App {
    pub(super) fn open_settings(&mut self, ctx: &egui::Context) {
        self.resolve_tab_input(ctx);
        self.confirm = Some(Confirm::Settings(SettingsDraft {
            values: self.settings.clone(),
            retro: self.retro.clone(),
            page: Page::Appearance,
            font_search: String::new(),
        }));
    }

    pub(super) fn open_about(&mut self, ctx: &egui::Context) {
        self.open_settings(ctx);
        if let Some(Confirm::Settings(draft)) = &mut self.confirm {
            draft.page = Page::About;
        }
    }

    pub(super) fn settings_dialog(&mut self, ctx: &egui::Context, mut draft: SettingsDraft) {
        let classic = self.experience() == Experience::Classic;
        let mut page = draft.page;
        let mut update_action = crate::updates::Action::None;
        let response = paintcraft_classic::settings::show(
            ctx,
            classic,
            &mut page,
            |ui, page, width, height| {
                update_action = content(
                    ui,
                    &mut draft,
                    &self.fonts.families,
                    page,
                    [width, height],
                    classic,
                    &mut self.updates,
                );
            },
        );
        draft.page = page;
        if update_action == crate::updates::Action::Restart {
            self.confirm = Some(Confirm::Settings(draft));
            self.begin_update_restart(ctx);
            return;
        }
        match response {
            Response::Cancel => {
                self.confirm = None;
                ctx.request_repaint();
            }
            Response::Apply => {
                let experience = draft.values.experience;
                draft.values.experience = self.experience();
                self.retro = draft.retro;
                self.retro.skin = draft.values.retro_skin;
                self.settings = draft.values;
                self.editor.options.pen = self.settings.pen;
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
    size: [f32; 2],
    classic: bool,
    updates: &mut crate::updates::State,
) -> crate::updates::Action {
    let [width, height] = size;
    let mut update_action = crate::updates::Action::None;
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
                        update_action=updates.show(ui);
                        egui::CollapsingHeader::new(t!(ui.ctx(), "Third-party licenses"))
                            .id_salt("third-party-licenses")
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .id_salt("third-party-license-text")
                                    .max_height(240.0)
                                    .show(ui, |ui| {
                                        ui.add(egui::Label::new(include_str!(
                                            "../../../../packaging/ThirdPartyLicenses.txt"
                                        )).selectable(true).wrap());
                                    });
                            });
                    }),
                    Page::Interface if draft.values.experience == Experience::Retro => card(ui, "Retro", |ui| {
                        ui.checkbox(&mut draft.retro.toolbox, t!(ui.ctx(), "Tools"));
                        ui.checkbox(&mut draft.retro.colors, t!(ui.ctx(), "Colors"));
                        ui.checkbox(&mut draft.retro.status, t!(ui.ctx(), "Status bar"));
                        ui.checkbox(&mut draft.retro.text_toolbar, t!(ui.ctx(), "Text toolbar"));
                    }),
                    Page::Interface => card(ui, "Canvas navigation", |ui| {
                        ui.checkbox(&mut draft.values.studio_auto_scroll, t!(ui.ctx(), "Auto-scroll"));
                        ui.label(t!(ui.ctx(), "Scroll automatically while dragging near the edge of the canvas viewport."));
                        ui.checkbox(&mut draft.values.studio_overscroll, t!(ui.ctx(), "Scroll past the edge of the image (overscroll)"));
                        ui.label(t!(ui.ctx(), "Move small images halfway offscreen and large image edges to the viewport center."));
                    }),
                    Page::Canvas => canvas(ui, &mut draft.values.canvas, classic, draft.values.experience == Experience::Retro),
                    Page::Pen => card(ui, "Pen and tablet", |ui| {
                        paintcraft_ui::pen_settings::show(ui, &mut draft.values.pen);
                    }),
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
    update_action
}

fn appearance(ui: &mut egui::Ui, values: &mut Settings) {
    card(ui, "App language", |ui| {
        picker(ui, &mut values.language);
    });
    card(ui, "App theme", |ui| {
        ui.weak(t!(ui.ctx(), "Changes take effect when you select Apply.",));
        if values.experience == Experience::Retro {
            ui.label(t!(
                ui.ctx(),
                "Retro uses a light palette. Your other interface themes are preserved."
            ));
            ui.add_enabled(false, egui::RadioButton::new(true, t!(ui.ctx(), "Light")));
            return;
        }
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
            ui.spacing_mut().item_spacing = egui::vec2(28.0, 12.0);
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
        ui.weak(t!(
            ui.ctx(),
            "Each interface has a separate workspace. Save or discard changes before switching."
        ));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(28.0, 12.0);
            for (value, label) in [
                (Experience::Classic, "Classic interface"),
                (Experience::Studio, "Studio interface"),
                (Experience::Retro, "Retro"),
            ] {
                ui.radio_value(&mut values.experience, value, t!(ui.ctx(), label));
            }
        });
    });
    if values.experience == Experience::Retro {
        card(ui, "Retro skin", |ui| {
            for skin in paintcraft_retro::Skin::ALL {
                ui.radio_value(&mut values.retro_skin, skin, t!(ui.ctx(), skin.name()));
            }
        });
    }
    if values.experience == Experience::Classic {
        card(ui, "Classic skin", |ui| {
            let mut variant = values.classic_variant.unwrap_or(ClassicVariant::DEFAULT);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(28.0, 12.0);
                for v in ClassicVariant::ALL {
                    if ui.radio_value(&mut variant, v, v.name()).changed() {
                        values.classic_variant = Some(variant);
                    }
                }
            });
        });
    }
    if values.experience == Experience::Classic {
        card(ui, "Canvas controls", |ui| {
            ui.checkbox(
                &mut values.classic_tool_controls,
                t!(ui.ctx(), "Show brush size and opacity bars"),
            );
        });
    }
}

fn canvas(ui: &mut egui::Ui, values: &mut settings::CanvasDefaults, classic: bool, retro: bool) {
    card(
        ui,
        if retro {
            "Canvas size"
        } else {
            "New Classic canvas"
        },
        |ui| {
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
        },
    );
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
