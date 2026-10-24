//! Historical Home ribbon: compact Clipboard, Image, Tools, Shapes and Colors groups.
use super::{Action, Controls, Workspace};
use egui::Vec2;
use paintcraft_ui::{actions::Context, i18n::tr, icons::Icon};

pub(super) fn show(
    ui: &mut egui::Ui,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: Context,
    actions: &mut Vec<Action>,
) {
    egui::ScrollArea::horizontal()
        .id_salt("classic-ribbon-scroll")
        .max_height(96.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                group(ui, "Clipboard", |ui| {
                    ui.horizontal_top(|ui| {
                        ui.add_enabled_ui(false, |ui| {
                            super::legacy_controls::large_button(
                                ui,
                                context,
                                Action::New,
                                Icon::Paste,
                                "Paste",
                            )
                            .on_disabled_hover_text(tr(
                                ui.ctx(),
                                "Canvas clipboard operations are not available yet.",
                            ));
                        });
                        ui.vertical(|ui| {
                            for (caption, icon) in [("Cut", Icon::Cut), ("Copy", Icon::Copy)] {
                                ui.add_enabled(
                                    false,
                                    egui::Button::new((
                                        paintcraft_ui::icons::legacy_image(ui.ctx(), icon, 14.0),
                                        tr(ui.ctx(), caption),
                                    ))
                                    .frame_when_inactive(false),
                                )
                                .on_disabled_hover_text(tr(
                                    ui.ctx(),
                                    "Canvas clipboard operations are not available yet.",
                                ));
                            }
                        });
                    });
                });
                ui.separator();
                group(ui, "Image", |ui| {
                    ui.horizontal_top(|ui| {
                        super::legacy_controls::selection(ui, state, context, actions);
                        ui.vertical(|ui| {
                            super::image_menus::legacy_controls(ui, context, actions);
                        });
                    });
                });
                ui.separator();
                group(ui, "Tools", |ui| {
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.add_space(5.0);
                            super::legacy_controls::tools(ui, state, context, actions);
                        });
                        brushes(ui, state, context, actions);
                    });
                });
                ui.separator();
                group(ui, "Shapes", |ui| {
                    ui.horizontal_top(|ui| {
                        super::shapes::legacy_gallery(ui, state, actions);
                        ui.vertical(|ui| {
                            super::legacy_controls::shape_styles(ui, state);
                        });
                        ui.add_space(4.0);
                        super::legacy_controls::size(ui, state, context);
                    });
                });
                ui.separator();
                group(ui, "Colors", |ui| {
                    super::legacy_palette::show(ui, state, workspace, context, actions)
                });
            });
        });
}

pub(super) fn group(ui: &mut egui::Ui, label: &str, contents: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        let top = ui.cursor().min.y;
        ui.set_min_height(90.0);
        contents(ui);
        ui.add_space((top + 76.0 - ui.cursor().min.y).max(0.0));
        let width = ui.min_rect().width();
        ui.allocate_ui_with_layout(
            Vec2::new(width, 12.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.label(
                    egui::RichText::new(tr(ui.ctx(), label))
                        .color(ui.visuals().text_color())
                        .size(11.0),
                );
            },
        );
    });
}

/// The brush image selects the tool; its lower half opens the media gallery.
fn brushes(ui: &mut egui::Ui, state: &mut Controls, context: Context, actions: &mut Vec<Action>) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let selected = state.tool == paintcraft_tools::Tool::Brush;
        let action = Action::SelectTool(paintcraft_tools::Tool::Brush);
        let response = ui.add_enabled(
            action.enabled(context),
            egui::Button::new(paintcraft_ui::icons::legacy_image(
                ui.ctx(),
                Icon::Brush,
                30.0,
            ))
            .min_size(Vec2::new(52.0, 40.0))
            .selected(selected)
            .frame_when_inactive(selected),
        );
        let label = tr(ui.ctx(), "Brush");
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::Role::Button,
                ui.is_enabled() && action.enabled(context),
                selected,
                &label,
            )
        });
        if response.on_hover_text(label).clicked() {
            state.tool = paintcraft_tools::Tool::Brush;
            actions.push(action);
        }
        let id = ui.make_persistent_id("legacy-brush-gallery-open");
        let was_open = ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
        let (button, popup) = egui::containers::menu::MenuButton::from_button(
            egui::Button::new(tr(ui.ctx(), "Brushes"))
                .min_size(Vec2::new(52.0, 28.0))
                .selected(selected)
                .frame_when_inactive(selected),
        )
        .ui(ui, |ui| {
            super::brush_gallery::gallery(ui, state, actions, !was_open);
        });
        ui.data_mut(|data| data.insert_temp(id, popup.is_some()));
        super::dropdowns::open_state(ui, &button, "Brushes");
        button.on_hover_text(tr(ui.ctx(), "Brushes"));
    });
}
