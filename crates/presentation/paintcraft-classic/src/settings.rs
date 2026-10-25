//! Settings presentation shared by native and browser hosts.
use egui::{Color32, Ui};
use paintcraft_ui::{icons::Icon, t};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Appearance,
    Interface,
    Canvas,
    Text,
    About,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Pending,
    Apply,
    Cancel,
}

pub fn show(
    ctx: &egui::Context,
    classic: bool,
    page: &mut Page,
    mut contents: impl FnMut(&mut Ui, Page, f32, f32),
) -> Response {
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
                                .size(28.0)
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
            let tabs = ui.horizontal_wrapped(|ui| navigation(ui, page, false, classic));
            ui.separator();
            body_height = (body_height - tabs.response.rect.height() - 12.0).max(60.0);
            contents(ui, *page, width, body_height);
        } else {
            ui.allocate_ui_with_layout(
                egui::vec2(width, body_height),
                egui::Layout::left_to_right(egui::Align::Min),
                |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(164.0, body_height),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            navigation(ui, page, true, classic);
                        },
                    );
                    ui.separator();
                    let content_width = ui.available_width();
                    contents(ui, *page, content_width, body_height);
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
                        egui::Button::new(egui::RichText::new(t!(ui.ctx(), "Apply")).color(ink))
                            .fill(ui.visuals().selection.stroke.color)
                            .min_size(egui::vec2(96.0, control_height)),
                    )
                    .clicked();
            },
        );
    });
    if ctx.current_pass_index() != 0 {
        return Response::Pending;
    }
    if cancel || response.should_close() {
        Response::Cancel
    } else if apply {
        Response::Apply
    } else {
        Response::Pending
    }
}

fn navigation(ui: &mut egui::Ui, current: &mut Page, sidebar: bool, classic: bool) {
    for (page, label, icon) in [
        (Page::Appearance, "Appearance", Icon::Appearance),
        (Page::Interface, "User Interface", Icon::Interface),
        (Page::Canvas, "Canvas", Icon::CanvasSize),
        (Page::Text, "Text preferences", Icon::Text),
        (Page::About, "About", Icon::About),
    ] {
        if classic && page == Page::Interface {
            continue;
        }
        let localized_label = t!(ui.ctx(), label);
        let label = localized_label.as_str();
        let image = if classic {
            match page {
                Page::Appearance | Page::Interface | Page::About => {
                    paintcraft_ui::icons::classic_menu_image(
                        ui.ctx(),
                        icon,
                        22.0,
                        ui.visuals().strong_text_color(),
                    )
                }
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
            paintcraft_ui::icons::image(ui.ctx(), icon, 18.0).tint(
                if matches!(page, Page::Appearance | Page::Interface | Page::About) {
                    ui.visuals().strong_text_color()
                } else {
                    Color32::WHITE
                },
            )
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
            crate::selected_button(ui, button, selected)
        } else {
            ui.add(button)
        };
        if response.clicked() {
            *current = page;
        }
    }
}

pub fn card(ui: &mut egui::Ui, heading: &str, contents: impl FnOnce(&mut egui::Ui)) {
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
            ui.label(
                egui::RichText::new(t!(ui.ctx(), heading))
                    .size(if heading == "About" { 22.0 } else { 18.0 })
                    .strong(),
            );
            ui.add_space(6.0);
            contents(ui);
        });
    ui.add_space(10.0);
}

/// Shared product identity in both application hosts.
pub fn about(ui: &mut Ui) {
    ui.label(t!(ui.ctx(), "PaintCraft by Replicraft"));
    ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
    ui.add(
        egui::Hyperlink::from_label_and_url(
            "GitHub: replicraft/paintcraft",
            "https://github.com/replicraft/paintcraft",
        )
        .open_in_new_tab(true),
    );
}
