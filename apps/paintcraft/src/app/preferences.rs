//! Application settings with reversible theme preview and explicit adoption.
use super::*;
use paintcraft_ui::i18n::picker;
use paintcraft_ui::icons::Icon;
use paintcraft_ui::t;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Page {
    Appearance,
    Interface,
    Canvas,
    Text,
}

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
        let mut apply = false;
        let mut cancel = false;
        let room = ctx.content_rect().size() - egui::vec2(48.0, 64.0);
        let width = room.x.clamp(240.0, 720.0);
        let height = room.y.clamp(180.0, 520.0);
        let control_height = if classic { 40.0 } else { 36.0 };
        let mut modal = egui::Modal::new(egui::Id::unique("application-settings"));
        if classic {
            modal = modal.frame(
                egui::Frame::window(&ctx.style_of(ctx.theme()))
                    .inner_margin(16)
                    .corner_radius(12),
            );
        }
        let response = modal.show(ctx, |ui| {
            ui.set_width(width);
            ui.set_max_height(height);
            ui.spacing_mut().interact_size.y = control_height;
            ui.spacing_mut().icon_width = 22.0;
            for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                ui.style_mut().text_styles.insert(
                    style,
                    egui::FontId::proportional(if classic { 16.0 } else { 15.0 }),
                );
            }
            if classic {
                ui.spacing_mut().scroll.floating_allocated_width = 4.0;
                ui.spacing_mut().scroll.dormant_handle_opacity = 0.6;
                ui.visuals_mut().override_text_color = Some(ui.visuals().strong_text_color());
                ui.visuals_mut().weak_text_color =
                    Some(Color32::from_gray(if ui.visuals().dark_mode {
                        184
                    } else {
                        88
                    }));
                ui.horizontal_top(|ui| {
                    let heading_width = width - control_height - ui.spacing().item_spacing.x;
                    ui.allocate_ui_with_layout(
                        egui::vec2(heading_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_width(heading_width);
                            ui.label(
                                egui::RichText::new(t!(ui.ctx(), "Settings"))
                                    .size(24.0)
                                    .strong(),
                            );
                        },
                    );
                    cancel = paintcraft_ui::widgets::classic_menu_button(
                        ui,
                        "Close settings",
                        Icon::Close,
                        false,
                        "Close settings",
                    )
                    .clicked();
                });
            } else {
                ui.heading(t!(ui.ctx(), "Settings"));
            }
            ui.add_space(10.0);
            let mut body_height = (height - if classic { 128.0 } else { 104.0 }).max(60.0);
            if width < 560.0 {
                let tabs =
                    ui.horizontal_wrapped(|ui| navigation(ui, &mut draft.page, false, classic));
                ui.separator();
                body_height = (body_height - tabs.response.rect.height() - 12.0).max(60.0);
                content(
                    ui,
                    &mut draft,
                    &self.fonts.families,
                    width,
                    body_height,
                    classic,
                );
            } else {
                ui.allocate_ui_with_layout(
                    egui::vec2(width, body_height),
                    egui::Layout::left_to_right(egui::Align::Min),
                    |ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(164.0, body_height),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                navigation(ui, &mut draft.page, true, classic);
                            },
                        );
                        ui.separator();
                        let content_width = ui.available_width();
                        content(
                            ui,
                            &mut draft,
                            &self.fonts.families,
                            content_width,
                            body_height,
                            classic,
                        );
                    },
                );
            }
            ui.add_space(10.0);
            ui.separator();
            ui.allocate_ui_with_layout(
                egui::vec2(width, control_height),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    cancel |= ui
                        .add(
                            egui::Button::new(t!(ui.ctx(), "Cancel"))
                                .min_size(egui::vec2(96.0, control_height)),
                        )
                        .clicked();
                    let ink = if ui.visuals().dark_mode {
                        Color32::from_gray(16)
                    } else {
                        Color32::WHITE
                    };
                    apply = ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(t!(ui.ctx(), "Apply")).color(ink),
                            )
                            .fill(ui.visuals().selection.stroke.color)
                            .min_size(egui::vec2(96.0, control_height)),
                        )
                        .clicked();
                },
            );
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

fn navigation(ui: &mut egui::Ui, current: &mut Page, sidebar: bool, classic: bool) {
    for (page, label, icon) in [
        (Page::Appearance, "Appearance", Icon::Settings),
        (Page::Interface, "User Interface", Icon::Settings),
        (Page::Canvas, "Canvas", Icon::CanvasSize),
        (Page::Text, "Text preferences", Icon::Text),
    ] {
        if classic && page == Page::Interface {
            continue;
        }
        let localized_label = t!(ui.ctx(), label);
        let label = localized_label.as_str();
        let image = if classic {
            match page {
                Page::Appearance | Page::Interface => paintcraft_ui::icons::classic_menu_image(
                    ui.ctx(),
                    icon,
                    22.0,
                    ui.visuals().strong_text_color(),
                ),
                Page::Canvas => paintcraft_ui::icons::classic_status_image(
                    ui.ctx(),
                    icon,
                    22.0,
                    ui.visuals().dark_mode,
                ),
                Page::Text => paintcraft_ui::icons::classic_tool_image(
                    ui.ctx(),
                    icon,
                    22.0,
                    ui.visuals().dark_mode,
                ),
            }
        } else {
            paintcraft_ui::icons::image(ui.ctx(), icon, 18.0)
        };
        let selected = *current == page;
        let mut button = if classic {
            egui::Button::new(())
                .left_text((label, image))
                .selected(selected)
                .frame_when_inactive(selected)
                .frame(true)
        } else {
            egui::Button::image_and_text(image, label)
                .selected(selected)
                .frame(selected)
        };
        if sidebar {
            button = button.min_size(egui::vec2(156.0, if classic { 40.0 } else { 36.0 }));
        }
        let response = if classic {
            paintcraft_classic::selected_button(ui, button, selected)
        } else {
            ui.add(button)
        };
        if response.clicked() {
            *current = page;
        }
    }
}

fn content(
    ui: &mut egui::Ui,
    draft: &mut SettingsDraft,
    fonts: &[String],
    width: f32,
    height: f32,
    classic: bool,
) {
    ui.allocate_ui_with_layout(egui::vec2(width, height), egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(width);
        if classic {
            paintcraft_classic::form_control_style(ui);
        }
        egui::ScrollArea::vertical().id_salt(("settings-page", draft.page))
            .max_height(height).auto_shrink([false, false]).show(ui, |ui| {
                match draft.page {
                    Page::Appearance => appearance(ui, &mut draft.values),
                    Page::Interface => card(ui, "Canvas navigation", |ui| {
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

fn card(ui: &mut egui::Ui, heading: &str, contents: impl FnOnce(&mut egui::Ui)) {
    let fill = if ui.visuals().dark_mode {
        Color32::from_gray(47)
    } else {
        Color32::WHITE
    };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(6)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width().max(0.0));
            ui.strong(t!(ui.ctx(), heading));
            ui.add_space(6.0);
            contents(ui);
        });
    ui.add_space(10.0);
}

fn appearance(ui: &mut egui::Ui, values: &mut Settings) {
    card(ui, "App language", |ui| {
        picker(ui, &mut values.language);
    });
    card(ui, "App theme", |ui| {
        ui.weak(t!(
            ui.ctx(),
            "Preview the theme now. Apply saves your choice; Cancel restores it.",
        ));
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
