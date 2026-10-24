//! Interactive presentation preview; no image transform is published here.
use egui::{Align, Color32, FontId, TextStyle, Ui, Vec2};
use paintcraft_ui::icons::Icon;

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    source: [u32; 2],
    pixels: bool,
    linked: bool,
    scale: [f64; 2],
    skew: [i16; 2],
}

impl Draft {
    pub fn new(size: (u32, u32)) -> Self {
        Self {
            source: [size.0.max(1), size.1.max(1)],
            pixels: false,
            linked: true,
            scale: [1.0; 2],
            skew: [0; 2],
        }
    }

    fn edit_size(&mut self, axis: usize, value: f64, maximum: u32) {
        let base = if self.pixels {
            f64::from(self.source[axis])
        } else {
            100.0
        };
        // Retain scale across unit changes; integer pixel readouts round only for display.
        let lower = 1.0 / f64::from(*self.source.iter().max().expect("two dimensions"));
        let upper = if self.linked {
            self.source
                .into_iter()
                .map(|n| f64::from(maximum) / f64::from(n))
                .fold(f64::INFINITY, f64::min)
        } else {
            f64::from(maximum) / f64::from(self.source[axis])
        };
        let scale = (value / base).clamp(lower, upper.max(lower));
        if self.linked {
            self.scale = [scale; 2];
        } else {
            self.scale[axis] = scale;
        }
    }
}

/// Returns true only for dismissal. OK stays disabled until transforms exist.
pub fn dialog(ctx: &egui::Context, draft: &mut Draft, maximum: u32) -> bool {
    let width = (ctx.content_rect().width() - 56.0).clamp(240.0, 400.0);
    let height = (ctx.content_rect().height() - 72.0).clamp(180.0, 540.0);
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::unique("resize-skew-preview"))
        .frame(
            egui::Frame::window(&ctx.style_of(ctx.theme()))
                .inner_margin(16)
                .corner_radius(12),
        )
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().interact_size.y = 40.0;
            ui.spacing_mut().icon_width = 24.0;
            ui.spacing_mut().icon_width_inner = 12.0;
            ui.visuals_mut().override_text_color = Some(ui.visuals().strong_text_color());
            ui.visuals_mut().weak_text_color =
                Some(Color32::from_gray(if ui.visuals().dark_mode {
                    184
                } else {
                    88
                }));
            ui.spacing_mut().button_padding.x = 12.0;
            ui.style_mut().drag_value_text_style = TextStyle::Body;
            for style in [TextStyle::Body, TextStyle::Button] {
                ui.style_mut()
                    .text_styles
                    .insert(style, FontId::proportional(16.0));
            }
            let header = ui.horizontal_top(|ui| {
                let heading = width - 40.0 - ui.spacing().item_spacing.x;
                ui.allocate_ui_with_layout(
                    Vec2::new(heading, 0.0),
                    egui::Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(heading);
                        ui.label(egui::RichText::new("Resize and skew").size(24.0).strong());
                    },
                );
                close = paintcraft_ui::widgets::classic_menu_button(
                    ui,
                    "Close resize and skew",
                    Icon::Close,
                    false,
                    "Close resize and skew",
                )
                .clicked();
            });
            ui.add_space(12.0);
            ui.spacing_mut().scroll.floating = false;
            egui::ScrollArea::vertical()
                .id_salt("resize-skew-body")
                .max_height((height - header.response.rect.height() - 110.0).max(48.0))
                .min_scrolled_height(48.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    crate::form_controls::style(ui);
                    ui.strong("Resize");
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 24.0;
                        ui.radio_value(&mut draft.pixels, false, "Percentage");
                        ui.radio_value(&mut draft.pixels, true, "Pixels");
                    });
                    ui.add_space(if ctx.content_rect().height() < 420.0 {
                        0.0
                    } else {
                        8.0
                    });
                    resize_fields(ui, draft, maximum);
                    ui.add_space(16.0);
                    ui.strong("Skew");
                    ui.horizontal_top(|ui| {
                        let width = (ui.available_width() - ui.spacing().item_spacing.x) * 0.5;
                        for (axis, label) in ["Horizontal", "Vertical"].into_iter().enumerate() {
                            field_column(ui, width, label, |ui| {
                                let response = ui.add(
                                    egui::DragValue::new(&mut draft.skew[axis])
                                        .range(-89..=89)
                                        .suffix("°")
                                        .min_size(Vec2::new(width, 40.0)),
                                );
                                numeric_info(
                                    ui,
                                    &response,
                                    &format!("Skew {}", label.to_lowercase()),
                                    f64::from(draft.skew[axis]),
                                );
                            });
                        }
                    });
                });
            ui.add_space(12.0);
            ui.label("Preview only")
                .on_hover_text("Resize and skew changes cannot be applied yet.");
            ui.separator();
            ui.horizontal(|ui| {
                let button = (width - ui.spacing().item_spacing.x) * 0.5;
                ui.add_enabled(
                    false,
                    egui::Button::new("OK").min_size(Vec2::new(button, 40.0)),
                )
                .on_disabled_hover_text("Applying resize and skew is not available yet.");
                close |= ui
                    .add_sized([button, 40.0], egui::Button::new("Cancel"))
                    .clicked();
            });
        });
    close || modal.should_close()
}

fn resize_fields(ui: &mut Ui, draft: &mut Draft, maximum: u32) {
    ui.horizontal_top(|ui| {
        let gap = ui.spacing().item_spacing.x;
        let width = (ui.available_width() - 40.0 - gap * 2.0) * 0.5;
        for (axis, label) in ["Horizontal", "Vertical"].into_iter().enumerate() {
            field_column(ui, width, label, |ui| {
                ui.push_id(("resize-field", axis, draft.pixels), |ui| {
                    let base = if draft.pixels {
                        f64::from(draft.source[axis])
                    } else {
                        100.0
                    };
                    let mut value = draft.scale[axis] * base;
                    let range = if draft.pixels {
                        value = value.round().clamp(1.0, f64::from(maximum));
                        1.0..=f64::from(maximum)
                    } else {
                        0.000001..=f64::from(maximum) * 100.0
                    };
                    let response = ui.add(
                        egui::DragValue::new(&mut value)
                            .range(range)
                            .max_decimals(if draft.pixels { 0 } else { 6 })
                            .min_size(Vec2::new(width, 40.0)),
                    );
                    numeric_info(
                        ui,
                        &response,
                        &format!("Resize {}", label.to_lowercase()),
                        value,
                    );
                    if response.changed() {
                        draft.edit_size(axis, value, maximum);
                    }
                });
            });
            if axis == 0 {
                ui.vertical(|ui| {
                    ui.add_space(ui.text_style_height(&TextStyle::Body) + gap);
                    let ink = if draft.linked {
                        if ui.visuals().dark_mode {
                            Color32::from_gray(16)
                        } else {
                            Color32::WHITE
                        }
                    } else {
                        ui.visuals().strong_text_color()
                    };
                    let button = egui::Button::image(paintcraft_ui::icons::classic_menu_image(
                        ui.ctx(),
                        Icon::AspectRatio,
                        22.0,
                        ink,
                    ))
                    .min_size(Vec2::splat(40.0))
                    .frame(true);
                    let button = if draft.linked {
                        button.fill(ui.visuals().selection.stroke.color)
                    } else {
                        button
                    };
                    let response = ui.add(button);
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::Role::Button,
                            ui.is_enabled(),
                            draft.linked,
                            "Maintain aspect ratio",
                        )
                    });
                    if response.on_hover_text("Maintain aspect ratio").clicked()
                        && ui.ctx().current_pass_index() == 0
                    {
                        draft.linked = !draft.linked;
                    }
                });
            }
        }
    });
}

fn field_column(ui: &mut Ui, width: f32, label: &str, contents: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, 0.0),
        egui::Layout::top_down(Align::Min),
        |ui| {
            ui.set_width(width);
            ui.label(label);
            contents(ui);
        },
    );
}

fn numeric_info(ui: &Ui, response: &egui::Response, label: &str, value: f64) {
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::Role::SpinButton, ui.is_enabled(), label);
        info.value = Some(value);
        info
    });
}
