//! Classic layer-list presentation. The desktop host applies every intention.
use egui::{Align, Layout, Vec2};
use paintcraft_ui::widgets::{Icon, icon_button};
use std::hash::Hash;

#[derive(Debug)]
pub struct Layer<'a, Id> {
    pub id: Id,
    pub name: &'a str,
    pub visible: bool,
    pub preview: Preview,
}

#[derive(Clone, Copy, Debug)]
pub enum Preview {
    Pending,
    Unavailable,
    Ready {
        texture: egui::TextureId,
        aspect: f32,
        uv: egui::Rect,
    },
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
    visible_previews: &mut Vec<Id>,
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
                        let selected = selected == Some(row.id);
                        let button = egui::Button::new(())
                            // left_text prepends its atoms, so supply the thumbnail last.
                            .left_text((
                                row.name,
                                egui::Atom::custom(egui::IdSalt::new("preview"), Vec2::splat(32.0)),
                            ))
                            .selected(selected)
                            .frame_when_inactive(selected)
                            .frame(true)
                            .min_size(Vec2::new(ui.available_width(), 40.0))
                            .truncate();
                        let painted = button.atom_ui(ui);
                        if let Some(rect) = painted.rect(egui::IdSalt::new("preview"))
                            && ui.is_rect_visible(rect)
                        {
                            visible_previews.push(row.id);
                            paint_preview(ui, rect, row.preview);
                        }
                        let hint = match row.preview {
                            Preview::Unavailable => {
                                format!("{}\nLayer preview unavailable", row.name)
                            }
                            _ => row.name.to_owned(),
                        };
                        let response = painted.response.on_hover_text(hint);
                        if selected {
                            ui.painter().rect_stroke(
                                response.rect.shrink(0.5),
                                5,
                                ui.visuals().selection.stroke,
                                egui::StrokeKind::Inside,
                            );
                        }
                        if response.clicked() {
                            actions.push(Action::Select(row.id));
                        }
                    });
                });
            }
        });
    actions
}

fn paint_preview(ui: &egui::Ui, rect: egui::Rect, preview: Preview) {
    let colors = if ui.visuals().dark_mode {
        [65, 95]
    } else {
        [205, 245]
    };
    for y in 0..8 {
        for x in 0..8 {
            let cell = egui::Rect::from_min_size(
                rect.min + Vec2::new(x as f32 * 4.0, y as f32 * 4.0),
                Vec2::splat(4.0),
            );
            ui.painter()
                .rect_filled(cell, 0, egui::Color32::from_gray(colors[(x + y) % 2]));
        }
    }
    match preview {
        Preview::Ready {
            texture,
            aspect,
            uv,
        } => {
            let size = if aspect >= 1.0 {
                Vec2::new(rect.width(), rect.height() / aspect)
            } else {
                Vec2::new(rect.width() * aspect, rect.height())
            };
            ui.painter().image(
                texture,
                egui::Rect::from_center_size(rect.center(), size),
                uv,
                egui::Color32::WHITE,
            );
        }
        Preview::Pending | Preview::Unavailable => {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                if matches!(preview, Preview::Pending) {
                    "…"
                } else {
                    "!"
                },
                egui::FontId::proportional(14.0),
                ui.visuals().text_color(),
            );
        }
    }
    ui.painter().rect_stroke(
        rect,
        1,
        ui.visuals().widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
}
