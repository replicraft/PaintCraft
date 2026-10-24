//! Historical two-column File panel. Navigation never writes; actions use the host outbox.
use super::Action;
use egui::{Color32, Rect, Response, Vec2};
use paintcraft_ui::t;
use paintcraft_ui::{
    actions::{Context, ImageExport},
    icons::Icon,
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Pane {
    #[default]
    Recent,
    SaveAs,
    Print,
    Background,
    About,
}

pub(super) fn show(ui: &mut egui::Ui, context: Context, actions: &mut Vec<Action>, opening: bool) {
    let id = ui.make_persistent_id("historical-file-pane");
    let mut pane = if opening {
        Pane::Recent
    } else {
        ui.data(|data| data.get_temp(id).unwrap_or_default())
    };
    let width = (ui.ctx().content_rect().width() - 32.0).clamp(120.0, 560.0);
    ui.set_width(width);
    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
    ui.spacing_mut().interact_size.y = 32.0;
    let height = super::popup_layout::height(ui, 40.0, 480.0);
    if width >= 360.0 {
        ui.horizontal_top(|ui| {
            egui::ScrollArea::vertical()
                .id_salt("historical-file-navigation")
                .max_width(190.0)
                .max_height(height)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_width(190.0);
                        navigation(ui, 190.0, context, actions, &mut pane);
                    });
                });
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("historical-file-content")
                .max_width((width - 210.0).max(120.0))
                .max_height(height)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_width((width - 210.0).max(120.0));
                        content(ui, context, actions, pane);
                    });
                });
        });
    } else {
        // Keep every operation reachable on narrow/short browser viewports.
        egui::ScrollArea::vertical()
            .id_salt("historical-file-stacked")
            .max_height(height)
            .show(ui, |ui| {
                navigation(ui, width, context, actions, &mut pane);
                ui.separator();
                content(ui, context, actions, pane);
            });
    }
    ui.data_mut(|data| data.insert_temp(id, pane));
}

fn navigation(
    ui: &mut egui::Ui,
    width: f32,
    context: Context,
    actions: &mut Vec<Action>,
    pane: &mut Pane,
) {
    for (action, icon, caption) in [
        (Action::New, Icon::New, "New"),
        (Action::Open, Icon::Open, "Open"),
        (Action::Save, Icon::Save, "Save"),
    ] {
        let response = row(ui, width, icon, caption, false, action.enabled(context));
        if response.hovered() {
            *pane = Pane::Recent;
        }
        dispatch(ui, response, action, actions);
    }
    category(ui, width, Icon::SaveAs, "Save as", Pane::SaveAs, pane);
    ui.separator();
    category(ui, width, Icon::Print, "Print", Pane::Print, pane);
    row(
        ui,
        width,
        Icon::ImportImage,
        "From scanner or camera…",
        false,
        false,
    )
    .on_disabled_hover_text(t!(
        ui.ctx(),
        "Scanner and camera acquisition is not available yet.",
    ));
    row(ui, width, Icon::Share, "Send in email", false, false).on_disabled_hover_text(t!(
        ui.ctx(),
        "Sending pictures by email is not available yet.",
    ));
    category(
        ui,
        width,
        Icon::DesktopBackground,
        "Set as desktop background",
        Pane::Background,
        pane,
    );
    ui.separator();
    let response = row(
        ui,
        width,
        Icon::ImageProperties,
        "Properties",
        false,
        Action::ImageProperties.enabled(context),
    );
    dispatch(ui, response, Action::ImageProperties, actions);
    category(ui, width, Icon::New, "About PaintCraft", Pane::About, pane);
    // Native projects and document tabs are PaintCraft extensions to flat Paint files.
    let response = row(
        ui,
        width,
        Icon::SaveAs,
        "Save as project",
        false,
        Action::SaveAs.enabled(context),
    );
    dispatch(ui, response, Action::SaveAs, actions);
    let response = row(
        ui,
        width,
        Icon::Close,
        "Close",
        false,
        Action::Close.enabled(context),
    );
    dispatch(ui, response, Action::Close, actions);
    ui.separator();
    let response = row(
        ui,
        width,
        Icon::Close,
        "Exit",
        false,
        Action::Exit.enabled(context),
    );
    dispatch(ui, response, Action::Exit, actions);
}

fn row(
    ui: &mut egui::Ui,
    width: f32,
    icon: Icon,
    caption: &str,
    selected: bool,
    enabled: bool,
) -> Response {
    let caption = t!(ui.ctx(), caption);
    let image = if matches!(
        icon,
        Icon::DesktopBackground | Icon::ImageProperties | Icon::ImportImage
    ) {
        paintcraft_ui::icons::classic_menu_image(ui.ctx(), icon, 22.0, ui.visuals().text_color())
    } else {
        paintcraft_ui::icons::legacy_image(ui.ctx(), icon, 22.0)
    };
    ui.add_enabled(
        enabled,
        egui::Button::new((image, caption))
            .min_size(Vec2::new(width, 32.0))
            .selected(selected)
            .frame_when_inactive(selected)
            .truncate(),
    )
}

fn category(
    ui: &mut egui::Ui,
    width: f32,
    icon: Icon,
    caption: &str,
    choice: Pane,
    pane: &mut Pane,
) {
    let response = row(ui, width, icon, caption, *pane == choice, true);
    let center = response.rect.right_center() - Vec2::new(7.0, 0.0);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            center + Vec2::new(-1.5, -3.0),
            center + Vec2::new(1.5, 0.0),
            center + Vec2::new(-1.5, 3.0),
        ],
        ui.visuals().text_color(),
        egui::Stroke::NONE,
    ));
    if response.hovered() || response.has_focus() || response.clicked() {
        *pane = choice;
    }
}

fn dispatch(ui: &mut egui::Ui, response: Response, action: Action, actions: &mut Vec<Action>) {
    if response.clicked() && ui.ctx().current_pass_index() == 0 {
        actions.push(action);
        ui.close();
    }
}

fn content(ui: &mut egui::Ui, context: Context, actions: &mut Vec<Action>, pane: Pane) {
    let title = match pane {
        Pane::Recent => "Recent pictures",
        Pane::SaveAs => "Save as",
        Pane::Print => "Print",
        Pane::Background => "Set as desktop background",
        Pane::About => "About PaintCraft",
    };
    ui.add(egui::Label::new(egui::RichText::new(t!(ui.ctx(), title)).strong()).wrap());
    ui.separator();
    match pane {
        Pane::Recent => {
            ui.add(egui::Label::new(t!(ui.ctx(), "Recent files are not available yet.")).wrap());
        }
        Pane::SaveAs => {
            for format in ImageExport::ALL {
                let action = Action::ExportImage(format);
                let response = format_row(
                    ui,
                    format.name(),
                    format.label(),
                    format.description(),
                    action.enabled(context),
                );
                dispatch(ui, response, action, actions);
            }
            format_row(
                ui,
                "GIF",
                "GIF picture",
                "GIF export is not available yet.",
                false,
            )
            .on_disabled_hover_text(t!(ui.ctx(), "GIF export is not available yet."));
            let response = format_row(
                ui,
                "…",
                "Other formats",
                "Choose PNG, BMP or JPEG in the save dialog.",
                Action::Export.enabled(context),
            );
            dispatch(ui, response, Action::Export, actions);
            let response = format_row(
                ui,
                "PCND",
                "PaintCraft project",
                "Keep editable layers and document data.",
                Action::SaveAs.enabled(context),
            );
            dispatch(ui, response, Action::SaveAs, actions);
        }
        Pane::Print => {
            for title in ["Print…", "Page setup…", "Print preview"] {
                row(ui, ui.available_width(), Icon::Print, title, false, false)
                    .on_disabled_hover_text(t!(ui.ctx(), "Printing is not available yet."));
            }
        }
        Pane::Background => {
            for title in ["Fill", "Tile", "Center"] {
                row(
                    ui,
                    ui.available_width(),
                    Icon::DesktopBackground,
                    title,
                    false,
                    false,
                )
                .on_disabled_hover_text(t!(
                    ui.ctx(),
                    "Setting the desktop background is not available yet.",
                ));
            }
        }
        Pane::About => {
            ui.heading("PaintCraft");
            ui.label(t!(
                ui.ctx(),
                "Version { $version }",
                &[("version", env!("CARGO_PKG_VERSION").into())],
            ));
            ui.label(t!(ui.ctx(), "A painting app for Classic and Studio."));
        }
    }
}

fn format_row(
    ui: &mut egui::Ui,
    format: &str,
    caption: &str,
    description: &str,
    enabled: bool,
) -> Response {
    let caption = t!(ui.ctx(), caption);
    let description = t!(ui.ctx(), description);
    let width = ui.available_width();
    let ink = if enabled {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let heading = ui.painter().layout(
        caption.clone(),
        egui::FontId::proportional(13.0),
        ink,
        (width - 50.0).max(24.0),
    );
    let details = ui.painter().layout(
        description.clone(),
        egui::FontId::proportional(11.0),
        ui.visuals().weak_text_color(),
        (width - 50.0).max(24.0),
    );
    let height = (heading.size().y + details.size().y + 12.0).max(58.0);
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("")
            .min_size(Vec2::new(width, height))
            .frame_when_inactive(false),
    );
    let icon = Rect::from_center_size(
        response.rect.left_top() + Vec2::new(21.0, 23.0),
        Vec2::splat(32.0),
    );
    paintcraft_ui::icons::legacy_image(ui.ctx(), Icon::New, 32.0)
        .tint(if enabled {
            Color32::WHITE
        } else {
            Color32::from_white_alpha(96)
        })
        .paint_at(ui, icon);
    ui.painter().text(
        icon.center() + Vec2::new(0.0, 4.0),
        egui::Align2::CENTER_CENTER,
        format,
        egui::FontId::proportional(8.0),
        Color32::from_rgb(25, 95, 160),
    );
    let pos = response.rect.left_top() + Vec2::new(44.0, 5.0);
    let heading_height = heading.size().y;
    ui.painter().galley(pos, heading, ink);
    ui.painter().galley(
        pos + Vec2::new(0.0, heading_height + 2.0),
        details,
        ui.visuals().weak_text_color(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, enabled && ui.is_enabled(), &caption)
    });
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_description(description));
    response
}
