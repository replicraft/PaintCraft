//! Classic layer-list presentation. The desktop host applies every intention.
use egui::{Align, Layout, Vec2};
use paintcraft_ui::widgets::{Icon, icon_button};
use std::hash::Hash;

#[derive(Debug)]
pub struct Layer<'a, Id> {
    pub id: Id,
    pub name: &'a str,
    pub visible: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action<Id> {
    Close,
    Add,
    Select(Id),
    SetVisible { layer: Id, visible: bool },
    MoveUp,
    MoveDown,
    Delete,
}

/// Rows are supplied topmost first. Reserve room below the list for the host's
/// name/opacity editors; scrolling must not push those controls off the panel.
pub fn panel<Id: Copy + Eq + Hash + std::fmt::Debug>(
    ui: &mut egui::Ui,
    rows: &[Layer<'_, Id>],
    selected: Option<Id>,
) -> Vec<Action<Id>> {
    let mut actions = Vec::new();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Layers").size(16.0).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, "Close Layers", Icon::Close, false, false).clicked() {
                actions.push(Action::Close);
            }
        });
    });
    ui.separator();
    let index = rows.iter().position(|row| Some(row.id) == selected);
    ui.horizontal(|ui| {
        for (label, icon, enabled, action) in [
            ("Add layer", Icon::AddLayer, true, Action::Add),
            (
                "Move up",
                Icon::MoveLayerUp,
                index.is_some_and(|i| i > 0),
                Action::MoveUp,
            ),
            (
                "Move down",
                Icon::MoveLayerDown,
                index.is_some_and(|i| i + 1 < rows.len()),
                Action::MoveDown,
            ),
            (
                "Delete layer",
                Icon::DeleteLayer,
                index.is_some(),
                Action::Delete,
            ),
        ] {
            if ui
                .add_enabled_ui(enabled, |ui| icon_button(ui, label, icon, false, false))
                .inner
                .clicked()
            {
                actions.push(action);
            }
        }
    });
    ui.separator();
    let height = (ui.available_height() - 120.0).max(40.0);
    egui::ScrollArea::vertical()
        .id_salt("classic-layer-list")
        .max_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            for row in rows {
                ui.push_id(row.id, |ui| {
                    ui.horizontal(|ui| {
                        let mut visible = row.visible;
                        let toggle = ui.checkbox(&mut visible, "").on_hover_text("Visible");
                        toggle.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::CheckBox,
                                ui.is_enabled(),
                                visible,
                                format!("Show {}", row.name),
                            )
                        });
                        if toggle.changed() {
                            actions.push(Action::SetVisible {
                                layer: row.id,
                                visible,
                            });
                        }
                        let button = egui::Button::new(())
                            .left_text(row.name)
                            .selected(selected == Some(row.id))
                            .min_size(Vec2::new(ui.available_width(), 40.0))
                            .truncate();
                        if ui.add(button).clicked() {
                            actions.push(Action::Select(row.id));
                        }
                    });
                });
            }
        });
    actions
}
