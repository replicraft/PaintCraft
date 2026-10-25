//! The Windows 7/10 ribbon has its own quick access strip and Home/View tabs.
use super::{Action, ClassicVariant, Controls, Workspace};
use egui::{Color32, Vec2};
use paintcraft_ui::t;
use paintcraft_ui::{
    actions::{Context, ViewAction},
    icons::Icon,
};

pub(super) fn show(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: Context,
    text: Option<super::TextControls<'_>>,
) -> Vec<Action> {
    let text_available = text.is_some() && state.tool == paintcraft_tools::Tool::Text;
    if workspace.legacy_tool != Some(state.tool) {
        workspace.legacy_tool = Some(state.tool);
        workspace.legacy_text = text_available;
        if text_available {
            workspace.legacy_view = false;
        }
    }
    let mut actions = Vec::new();
    ui.visuals_mut().menu_corner_radius = egui::CornerRadius::ZERO;
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
        if text_available {
            let fill = if ui.visuals().dark_mode {
                Color32::from_rgb(43, 77, 54)
            } else {
                Color32::from_rgb(210, 235, 216)
            };
            egui::Frame::new()
                .fill(fill)
                .inner_margin(egui::Margin::symmetric(8, 2))
                .show(ui, |ui| {
                    ui.label(t!(ui.ctx(), "Text Tools"));
                });
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
                .selectable_label(workspace.layers, t!(ui.ctx(), "Layers"))
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
            egui::Button::new(" ")
                .fill(Color32::from_rgb(38, 89, 149))
                .min_size(Vec2::new(42.0, 24.0))
        } else {
            egui::Button::new(egui::RichText::new(t!(ui.ctx(), "File")).color(Color32::WHITE))
                .fill(Color32::from_rgb(25, 121, 202))
                .min_size(Vec2::new(56.0, 24.0))
        };
        let state_id = ui.make_persistent_id("historical-file-open");
        let was_open = ui.data(|data| data.get_temp::<bool>(state_id).unwrap_or(false));
        let (response, popup) = egui::containers::menu::MenuButton::from_button(file)
            .config(
                egui::containers::menu::MenuConfig::new()
                    .style(|style: &mut egui::Style| {
                        egui::containers::menu::menu_style(style);
                        style.visuals.menu_corner_radius = egui::CornerRadius::ZERO;
                    })
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
            )
            .ui(ui, |ui| {
                super::legacy_file::show(ui, context, &mut actions, !was_open)
            });
        ui.data_mut(|data| data.insert_temp(state_id, popup.is_some()));
        if variant == ClassicVariant::V7 {
            let glyph = egui::Rect::from_min_size(
                response.rect.min + Vec2::new(10.0, 7.0),
                Vec2::new(13.0, 10.0),
            );
            ui.painter()
                .rect_filled(glyph, 0, Color32::from_rgb(239, 246, 255));
            ui.painter().rect_filled(
                egui::Rect::from_min_max(
                    glyph.min + Vec2::splat(1.0),
                    glyph.min + Vec2::new(4.0, 9.0),
                ),
                0,
                Color32::from_rgb(103, 160, 216),
            );
            for offset in [2.5, 5.0, 7.5] {
                ui.painter().line_segment(
                    [
                        glyph.min + Vec2::new(6.0, offset),
                        glyph.min + Vec2::new(11.0, offset),
                    ],
                    egui::Stroke::new(0.8, Color32::from_rgb(70, 102, 138)),
                );
            }
            super::legacy_controls::arrow(ui, response.rect.right_center() - Vec2::new(7.0, 0.0));
        }
        let label = t!(ui.ctx(), "File");
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &label));
        for (view, title) in [(false, "Home"), (true, "View")] {
            let selected = !workspace.legacy_text && workspace.legacy_view == view;
            let response = ui.add(
                egui::Button::new(t!(ui.ctx(), title))
                    .fill(if selected {
                        if variant == ClassicVariant::V7 && !ui.visuals().dark_mode {
                            Color32::from_rgb(250, 252, 255)
                        } else {
                            ui.visuals().panel_fill
                        }
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
                workspace.legacy_text = false;
            }
        }
        if text_available {
            let selected = workspace.legacy_text;
            let response = ui.add(
                egui::Button::new(t!(ui.ctx(), "Text"))
                    .fill(if selected {
                        ui.visuals().panel_fill
                    } else if ui.visuals().dark_mode {
                        Color32::from_rgb(43, 77, 54)
                    } else {
                        Color32::from_rgb(210, 235, 216)
                    })
                    .min_size(Vec2::new(58.0, 24.0)),
            );
            let label = t!(ui.ctx(), "Text");
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::Tab, ui.is_enabled(), selected, &label)
            });
            if response.clicked() {
                workspace.legacy_text = true;
                workspace.legacy_view = false;
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
        if workspace.legacy_text && text_available {
            egui::ScrollArea::horizontal()
                .id_salt("historical-text-ribbon")
                .max_height(88.0)
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        text.expect("text controls are available")
                            .historical(ui, variant);
                        ui.separator();
                        super::legacy_ribbon::group(ui, "Colors", |ui| {
                            super::legacy_palette::show(
                                ui,
                                variant,
                                state,
                                workspace,
                                context,
                                &mut actions,
                            )
                        });
                    });
                });
        } else if workspace.legacy_view {
            view_ribbon(ui, context, &mut actions);
        } else {
            super::legacy_ribbon::show(ui, variant, state, workspace, context, &mut actions);
        }
        if variant == ClassicVariant::V7 {
            let rect = egui::Rect::from_min_max(
                bounds.min,
                egui::pos2(bounds.right(), ui.min_rect().bottom()),
            );
            let mut mesh = egui::epaint::Mesh::default();
            let (top, bottom) = if ui.visuals().dark_mode {
                (Color32::from_rgb(47, 64, 88), Color32::from_rgb(32, 47, 67))
            } else {
                (
                    Color32::from_rgb(250, 252, 255),
                    Color32::from_rgb(223, 233, 246),
                )
            };
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
                            t!(ui.ctx(), action.descriptor_for(context.frontend).label),
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
