//! Historical Home ribbon: compact Clipboard, Image, Tools, Shapes and Colors groups.
use super::{Action, ClassicVariant, Controls, Workspace};
use egui::Vec2;
use paintcraft_ui::{actions::Context, i18n::tr, icons::Icon};

pub(super) fn show(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: Context,
    actions: &mut Vec<Action>,
) {
    egui::ScrollArea::horizontal()
        .id_salt("classic-ribbon-scroll")
        .max_height(88.0)
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
                        super::legacy_controls::selection(ui, variant, state, context, actions);
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
                            super::legacy_controls::tools(ui, variant, state, context, actions);
                        });
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    brushes(ui, variant, state, context, actions);
                });
                ui.separator();
                group(ui, "Shapes", |ui| {
                    ui.horizontal_top(|ui| {
                        super::shapes::legacy_gallery(ui, variant, state, actions);
                        ui.vertical(|ui| {
                            super::legacy_controls::shape_styles(ui, state);
                        });
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    super::legacy_size::show(ui, variant, state, context);
                });
                ui.separator();
                group(ui, "Colors", |ui| {
                    super::legacy_palette::show(ui, variant, state, workspace, context, actions)
                });
            });
        });
}

pub(super) fn group(ui: &mut egui::Ui, label: &str, contents: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        let top = ui.cursor().min.y;
        ui.set_min_height(82.0);
        contents(ui);
        ui.add_space((top + 68.0 - ui.cursor().min.y).max(0.0));
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
fn brushes(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    state: &mut Controls,
    context: Context,
    actions: &mut Vec<Action>,
) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let selected = state.tool == paintcraft_tools::Tool::Brush;
        let action = Action::SelectTool(paintcraft_tools::Tool::Brush);
        let response = ui
            .add_enabled_ui(action.enabled(context), |ui| {
                super::legacy_surface::button(
                    ui,
                    variant,
                    egui::Button::new(paintcraft_ui::icons::legacy_image(
                        ui.ctx(),
                        Icon::Brush,
                        30.0,
                    ))
                    .min_size(Vec2::new(52.0, 40.0))
                    .selected(selected)
                    .frame_when_inactive(selected),
                    selected,
                )
            })
            .inner;
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
            egui::Button::new(" ")
                .min_size(Vec2::new(52.0, 24.0))
                .selected(selected)
                .frame_when_inactive(selected),
        )
        .config(super::legacy_surface::menu_config())
        .ui(ui, |ui| {
            super::legacy_brushes::gallery(ui, variant, state, actions, !was_open);
        });
        ui.data_mut(|data| data.insert_temp(id, popup.is_some()));
        ui.painter().text(
            button.rect.center_top() + Vec2::new(0.0, 6.0),
            egui::Align2::CENTER_CENTER,
            tr(ui.ctx(), "Brushes"),
            egui::TextStyle::Body.resolve(ui.style()),
            ui.visuals().text_color(),
        );
        super::legacy_controls::arrow(ui, button.rect.center_bottom() - Vec2::new(0.0, 5.0));
        super::legacy_surface::open_state(ui, &button, "Brushes");
        button.on_hover_text(tr(ui.ctx(), "Brushes"));
    });
}
