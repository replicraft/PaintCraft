//! Responsive skin choice presentation. The host owns drafts and persistence.
use super::ClassicVariant;
use egui::{Align2, Color32, FontId, Rect, Vec2};
use paintcraft_ui::t;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Pending,
    Apply,
    Cancel,
}

pub fn show(ctx: &egui::Context, current: &mut ClassicVariant, first_visit: bool) -> Choice {
    let width = (ctx.content_rect().width() - 56.0).clamp(240.0, 540.0);
    let height = (ctx.content_rect().height() - 72.0).clamp(180.0, 480.0);
    let wide = width >= 480.0;
    let mut choice = Choice::Pending;
    let id = egui::Id::unique("classic-skin-chooser");
    let layer = egui::LayerId::new(egui::Order::Foreground, id);
    ctx.memory_mut(|memory| memory.set_modal_layer(layer));
    ctx.layer_painter(layer)
        .rect_filled(ctx.content_rect(), 0, Color32::from_black_alpha(96));
    egui::Window::new(t!(ctx, "Choose your Classic skin"))
        .id(id)
        .order(egui::Order::Foreground)
        .title_bar(false)
        .frame(
            egui::Frame::window(&ctx.style_of(ctx.theme()))
                .inner_margin(16)
                .corner_radius(12),
        )
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.set_max_height(height);
            for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                ui.style_mut()
                    .text_styles
                    .insert(style, FontId::proportional(16.0));
            }
            ui.visuals_mut().override_text_color = Some(ui.visuals().strong_text_color());
            ui.spacing_mut().interact_size.y = 40.0;
            let header = ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(width - 48.0, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(width - 48.0);
                        ui.label(
                            egui::RichText::new(t!(
                                ui.ctx(),
                                "Choose your Classic skin",
                            ))
                            .size(24.0)
                            .strong(),
                        );
                    },
                );
                if paintcraft_ui::widgets::classic_menu_button(
                    ui,
                    "Close skin chooser",
                    paintcraft_ui::icons::Icon::Close,
                    false,
                    "Close skin chooser",
                )
                .clicked()
                {
                    choice = Choice::Cancel;
                }
            });
            ui.add_space(8.0);
            ui.spacing_mut().scroll.floating = false;
            egui::ScrollArea::vertical()
                .id_salt("classic-skin-choices")
                .max_height(
                    (height - header.response.rect.height() - 84.0)
                        .max(48.0)
                        .min(if wide { 220.0 } else { 400.0 }),
                )
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.add(egui::Label::new(
                        t!(ui.ctx(), "Choose a Paint skin. Change it later from the changer button or Settings."),
                    ).wrap());
                    ui.add_space(12.0);
                    if wide {
                        let card_width = (ui.available_width() - 24.0) / 3.0;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            for skin in ClassicVariant::ALL {
                                card(ui, current, skin, card_width, true);
                            }
                        });
                    } else {
                        for skin in ClassicVariant::ALL {
                            card(ui, current, skin, ui.available_width(), false);
                            ui.add_space(8.0);
                        }
                    }
                });
            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                let button_width = (width - ui.spacing().item_spacing.x) / 2.0;
                let ink = if ui.visuals().dark_mode {
                    Color32::BLACK
                } else {
                    Color32::WHITE
                };
                let apply = egui::Button::new(
                    egui::RichText::new(t!(ui.ctx(), if first_visit { "Continue" } else { "Apply" })).color(ink),
                )
                .fill(ui.visuals().selection.stroke.color);
                if ui.add_sized([button_width, 40.0], apply).clicked() {
                    choice = Choice::Apply;
                }
                if ui
                    .add_sized(
                        [button_width, 40.0],
                        egui::Button::new(t!(ui.ctx(), "Cancel")),
                    )
                    .clicked()
                {
                    choice = Choice::Cancel;
                }
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        Choice::Cancel
    } else {
        choice
    }
}

fn card(
    ui: &mut egui::Ui,
    current: &mut ClassicVariant,
    skin: ClassicVariant,
    width: f32,
    wide: bool,
) {
    let selected = *current == skin;
    let response = ui.add(
        egui::Button::new(" ")
            .selected(selected)
            .frame_when_inactive(selected)
            .min_size(Vec2::new(width, if wide { 132.0 } else { 78.0 })),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::Role::RadioButton,
            ui.is_enabled(),
            selected,
            skin.name(),
        )
    });
    if response.clicked() {
        *current = skin;
    }
    let thumbnail = if wide {
        Rect::from_min_size(
            response.rect.min + Vec2::splat(10.0),
            Vec2::new(width - 20.0, 86.0),
        )
    } else {
        Rect::from_center_size(
            response.rect.left_center() + Vec2::new(54.0, 0.0),
            Vec2::new(88.0, 58.0),
        )
    };
    draw_thumbnail(ui, thumbnail, skin);
    let (position, align) = if wide {
        (
            response.rect.center_bottom() - Vec2::new(0.0, 16.0),
            Align2::CENTER_CENTER,
        )
    } else {
        (
            response.rect.left_center() + Vec2::new(112.0, 0.0),
            Align2::LEFT_CENTER,
        )
    };
    ui.painter().text(
        position,
        align,
        skin.name(),
        FontId::proportional(16.0),
        ui.visuals().text_color(),
    );
    if selected {
        ui.painter().rect_stroke(
            response.rect.shrink(0.5),
            6,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
}

/// Original schematics illustrate layout choices; they are not rendered previews.
fn draw_thumbnail(ui: &egui::Ui, rect: Rect, skin: ClassicVariant) {
    let painter = ui.painter();
    let point = |x: f32, y: f32| rect.min + Vec2::new(rect.width() * x, rect.height() * y);
    let dark = ui.visuals().dark_mode;
    let bar = if skin == ClassicVariant::V7 {
        if dark {
            Color32::from_rgb(40, 65, 100)
        } else {
            Color32::from_rgb(120, 160, 215)
        }
    } else {
        ui.visuals().window_fill()
    };
    let round = if skin == ClassicVariant::V11 { 6 } else { 0 };
    painter.rect_filled(rect, round, ui.visuals().extreme_bg_color);
    painter.rect_filled(
        Rect::from_min_max(point(0.0, 0.0), point(1.0, 0.16)),
        round,
        bar,
    );
    painter.rect_filled(
        Rect::from_min_max(point(0.0, 0.16), point(1.0, 0.48)),
        0,
        ui.visuals().panel_fill,
    );
    for i in 0..6 {
        painter.rect_filled(
            Rect::from_min_max(
                point(0.06 + i as f32 * 0.14, 0.23),
                point(0.14 + i as f32 * 0.14, 0.37),
            ),
            if skin == ClassicVariant::V11 { 3 } else { 0 },
            ui.visuals().widgets.noninteractive.fg_stroke.color,
        );
    }
    painter.rect_filled(
        Rect::from_min_max(point(0.06, 0.57), point(0.94, 0.92)),
        0,
        Color32::WHITE,
    );
    painter.rect_stroke(
        rect,
        round,
        ui.visuals().widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
}
