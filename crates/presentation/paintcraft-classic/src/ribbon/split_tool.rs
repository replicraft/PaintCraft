//! Joined primary tool/menu controls; menu contents and editing remain host-owned.
use super::*;
use egui::{Color32, CornerRadius, Response, Shape, Stroke, StrokeKind};

pub(super) fn show(
    ui: &mut egui::Ui,
    state: &mut Controls,
    tool: Tool,
    context: Context,
    actions: &mut Vec<Action>,
) {
    let (icon, label) = match tool {
        Tool::Select => (Icon::RectangleSelect, "Selection options"),
        Tool::Brush => (Icon::Brush, "Brushes"),
        _ => unreachable!("only Selection and Brushes have primary split controls"),
    };
    let localized_tool = paintcraft_ui::i18n::tr(ui.ctx(), tool.name());
    let localized_menu = paintcraft_ui::i18n::tr(ui.ctx(), label);
    ui.allocate_ui_with_layout(
        Vec2::new(64.0, 80.0),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.set_width(64.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(12.0);
            let background = ui.painter().add(Shape::Noop);
            let arrow_background = ui.painter().add(Shape::Noop);
            let divider = ui.painter().add(Shape::Noop);
            let selected = state.tool == tool;
            let main = ui.add(
                egui::Button::new((
                    egui::Atom::grow(),
                    paintcraft_ui::icons::classic_menu_image(
                        ui.ctx(),
                        icon,
                        32.0,
                        ui.visuals().strong_text_color(),
                    ),
                    egui::Atom::grow(),
                ))
                .min_size(Vec2::splat(48.0))
                .selected(selected)
                .frame(false),
            );
            main.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::Role::Button,
                    ui.is_enabled(),
                    selected,
                    &localized_tool,
                )
            });
            if main.clicked() && ui.ctx().current_pass_index() == 0 {
                state.tool = tool;
                actions.push(Action::SelectTool(tool));
                ui.ctx().request_repaint();
            }
            ui.spacing_mut().interact_size.y = 20.0;
            ui.spacing_mut().button_padding.y = 0.0;
            let open_state = ui.make_persistent_id((label, "was-open"));
            let was_open = ui.data(|data| data.get_temp::<bool>(open_state).unwrap_or(false));
            let arrow = ui
                .push_id(label, |ui| {
                    egui::containers::menu::MenuButton::from_button(
                        egui::Button::new(" ")
                            .min_size(Vec2::new(48.0, 20.0))
                            .frame(false),
                    )
                    .config(
                        egui::containers::menu::MenuConfig::new()
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                    )
                    .ui(ui, |ui| match tool {
                        Tool::Select => crate::selection_menu::show(ui, state, context, actions),
                        Tool::Brush => crate::brush_gallery::gallery(ui, state, actions, !was_open),
                        _ => unreachable!(),
                    })
                    .0
                })
                .inner;
            let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::menu(&arrow).get_id());
            ui.data_mut(|data| data.insert_temp(open_state, open));
            arrow.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::Role::Button,
                    ui.is_enabled(),
                    open,
                    &localized_menu,
                )
            });
            let body = main.rect.union(arrow.rect);
            let main_hover = main.hovered() || main.has_focus();
            if selected || main_hover {
                let stroke = if selected && ui.is_enabled() {
                    ui.visuals().selection.stroke
                } else {
                    ui.visuals().widgets.hovered.bg_stroke
                };
                ui.painter().set(
                    background,
                    egui::epaint::RectShape::new(
                        body.shrink(0.5),
                        5,
                        crate::control_surface::fill(
                            ui,
                            main_hover,
                            main.is_pointer_button_down_on(),
                        ),
                        stroke,
                        StrokeKind::Inside,
                    ),
                );
            }
            if arrow.hovered() || arrow.has_focus() || open {
                let stroke = if selected {
                    Stroke::NONE
                } else {
                    ui.visuals().widgets.hovered.bg_stroke
                };
                ui.painter().set(
                    arrow_background,
                    egui::epaint::RectShape::new(
                        arrow.rect.shrink(1.0),
                        CornerRadius {
                            nw: 0,
                            ne: 0,
                            sw: 4,
                            se: 4,
                        },
                        crate::control_surface::fill(ui, true, arrow.is_pointer_button_down_on()),
                        stroke,
                        StrokeKind::Inside,
                    ),
                );
                if selected || main_hover {
                    ui.painter().set(
                        divider,
                        Shape::line_segment(
                            [
                                arrow.rect.left_top() + Vec2::new(1.0, 0.0),
                                arrow.rect.right_top() - Vec2::new(1.0, 0.0),
                            ],
                            ui.visuals().widgets.noninteractive.bg_stroke,
                        ),
                    );
                }
            }
            chevron(ui, &arrow);
            main.on_hover_text(paintcraft_ui::i18n::tr(ui.ctx(), tool.name()));
            arrow.on_hover_text(paintcraft_ui::i18n::tr(ui.ctx(), label));
        },
    );
}

fn chevron(ui: &egui::Ui, response: &Response) {
    let center = response.rect.center();
    let ink = if ui.is_enabled() {
        ui.visuals().text_color()
    } else {
        Color32::from_gray(128)
    };
    ui.painter().add(Shape::line(
        vec![
            center + Vec2::new(-3.0, -1.5),
            center + Vec2::new(0.0, 1.5),
            center + Vec2::new(3.0, -1.5),
        ],
        Stroke::new(1.0, ink),
    ));
}
