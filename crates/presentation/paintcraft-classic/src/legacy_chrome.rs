//! The Windows 7/10 ribbon has its own quick access strip and Home/View tabs.
use super::{Action, ClassicVariant, Controls, Workspace};
use egui::{Color32, Vec2};
use paintcraft_ui::{
    actions::{Context, ViewAction},
    i18n::tr,
    icons::Icon,
};

pub(super) fn show(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: Context,
) -> Vec<Action> {
    let mut actions = Vec::new();
    for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
        ui.style_mut()
            .text_styles
            .insert(style, egui::FontId::proportional(12.0));
    }
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
    ui.spacing_mut().item_spacing = Vec2::new(3.0, 1.0);
    ui.spacing_mut().button_padding = Vec2::new(4.0, 2.0);
    ui.spacing_mut().interact_size = Vec2::splat(20.0);
    let quick = ui.horizontal(|ui| {
        for action in [
            Action::Save,
            Action::Undo,
            Action::Redo,
            Action::ClearCanvas,
        ] {
            let icon = match action {
                Action::Save => Icon::Save,
                Action::Undo => Icon::Undo,
                Action::Redo => Icon::Redo,
                _ => Icon::ClearCanvas,
            };
            if super::legacy_controls::icon_button(ui, context, action, icon, 18.0).clicked() {
                actions.push(action);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if super::legacy_controls::icon_button(
                ui,
                context,
                Action::Settings,
                Icon::Settings,
                18.0,
            )
            .clicked()
            {
                actions.push(Action::Settings);
            }
            actions.extend(paintcraft_ui::chrome::interface_changer(ui, context));
            if ui
                .selectable_label(workspace.layers, tr(ui.ctx(), "Layers"))
                .clicked()
                && ui.ctx().current_pass_index() == 0
            {
                workspace.layers = !workspace.layers;
            }
        });
    });
    let tabs = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let file = if variant == ClassicVariant::V7 {
            egui::Button::new((
                paintcraft_ui::icons::legacy_image(ui.ctx(), Icon::New, 14.0),
                "  ",
            ))
            .fill(Color32::from_rgb(38, 89, 149))
            .min_size(Vec2::new(42.0, 24.0))
        } else {
            egui::Button::new(egui::RichText::new(tr(ui.ctx(), "File")).color(Color32::WHITE))
                .fill(Color32::from_rgb(25, 121, 202))
                .min_size(Vec2::new(56.0, 24.0))
        };
        let (response, _) = egui::containers::menu::MenuButton::from_button(file).ui(ui, |ui| {
            super::menus::file(ui, context, &mut actions);
        });
        if variant == ClassicVariant::V7 {
            super::legacy_controls::arrow(ui, response.rect.right_center() - Vec2::new(7.0, 0.0));
        }
        let label = tr(ui.ctx(), "File");
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &label));
        for (view, title) in [(false, "Home"), (true, "View")] {
            let selected = workspace.legacy_view == view;
            let response = ui.add(
                egui::Button::new(tr(ui.ctx(), title))
                    .fill(if selected {
                        ui.visuals().panel_fill
                    } else {
                        Color32::TRANSPARENT
                    })
                    .stroke(if selected {
                        ui.visuals().widgets.noninteractive.bg_stroke
                    } else {
                        egui::Stroke::NONE
                    })
                    .frame_when_inactive(selected)
                    .min_size(Vec2::new(58.0, 24.0)),
            );
            if response.clicked() {
                workspace.legacy_view = view;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            super::toolbar_visibility::menu(ui, context, &mut actions);
        });
    });
    if workspace.toolbar.update(
        ui,
        quick.response.rect.union(tabs.response.rect),
        context.auto_hide_toolbar,
    ) {
        let bounds = ui.available_rect_before_wrap();
        let background = ui.painter().add(egui::Shape::Noop);
        if workspace.legacy_view {
            view_ribbon(ui, context, &mut actions);
        } else {
            super::legacy_ribbon::show(ui, state, workspace, context, &mut actions);
        }
        if variant == ClassicVariant::V7 && !ui.visuals().dark_mode {
            let rect = egui::Rect::from_min_max(
                bounds.min,
                egui::pos2(bounds.right(), ui.min_rect().bottom()),
            );
            let mut mesh = egui::epaint::Mesh::default();
            let top = Color32::from_rgb(250, 252, 255);
            let bottom = Color32::from_rgb(223, 233, 246);
            for (point, color) in [
                (rect.left_top(), top),
                (rect.right_top(), top),
                (rect.right_bottom(), bottom),
                (rect.left_bottom(), bottom),
            ] {
                mesh.colored_vertex(point, color);
            }
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            ui.painter().set(background, egui::Shape::mesh(mesh));
        }
    }
    actions
}

fn view_ribbon(ui: &mut egui::Ui, context: Context, actions: &mut Vec<Action>) {
    ui.horizontal_top(|ui| {
        super::legacy_ribbon::group(ui, "Zoom", |ui| {
            ui.horizontal_top(|ui| {
                for (view, icon) in [
                    (ViewAction::ZoomIn, Icon::ZoomIn),
                    (ViewAction::ZoomOut, Icon::ZoomOut),
                    (ViewAction::ActualSize, Icon::ActualSize),
                    (ViewAction::FitWindow, Icon::FitWindow),
                ] {
                    let action = Action::View(view);
                    if super::legacy_controls::large_button(
                        ui,
                        context,
                        action,
                        icon,
                        action.descriptor().label,
                    )
                    .clicked()
                    {
                        actions.push(action);
                    }
                }
            });
        });
        ui.separator();
        super::legacy_ribbon::group(ui, "Show or hide", |ui| {
            for (view, checked) in [
                (ViewAction::Rulers, context.rulers),
                (ViewAction::PixelGrid, context.pixel_grid),
                (ViewAction::StatusBar, context.status_bar),
            ] {
                let action = Action::View(view);
                let mut value = checked;
                if ui
                    .add_enabled(
                        action.enabled(context),
                        egui::Checkbox::new(
                            &mut value,
                            tr(ui.ctx(), action.descriptor_for(context.frontend).label),
                        ),
                    )
                    .changed()
                {
                    actions.push(action);
                }
            }
        });
        ui.separator();
        super::legacy_ribbon::group(ui, "Display", |ui| {
            ui.horizontal_top(|ui| {
                for (view, icon) in [
                    (ViewAction::FullScreen, Icon::FitWindow),
                    (ViewAction::Thumbnail, Icon::Thumbnail),
                ] {
                    let action = Action::View(view);
                    if super::legacy_controls::large_button(
                        ui,
                        context,
                        action,
                        icon,
                        action.descriptor().label,
                    )
                    .clicked()
                    {
                        actions.push(action);
                    }
                }
            });
        });
    });
}
