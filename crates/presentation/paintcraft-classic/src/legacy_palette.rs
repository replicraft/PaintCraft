//! Square Color 1/Color 2 wells and a three-row palette, matching the old ribbon.
use super::{Action, ClassicVariant, Controls, PALETTE, Rgba, Workspace};
use egui::{Color32, Rect, Vec2};
use paintcraft_ui::{
    actions::Context,
    i18n::{tr, tr_format},
    icons::Icon,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PaletteTarget {
    #[default]
    Primary,
    Secondary,
}
impl PaletteTarget {
    fn edit(self) -> Action {
        match self {
            Self::Primary => Action::EditColors,
            Self::Secondary => Action::EditSecondaryColors,
        }
    }
    fn custom(self, slot: u8) -> Action {
        match self {
            Self::Primary => Action::EditCustomColor(slot),
            Self::Secondary => Action::EditSecondaryCustomColor(slot),
        }
    }
    fn apply(self, state: &mut Controls, color: Rgba) {
        match self {
            Self::Primary => state.primary = color,
            Self::Secondary => state.secondary = color,
        }
    }
}

pub(super) fn show(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: Context,
    actions: &mut Vec<Action>,
) {
    ui.horizontal_top(|ui| {
        for (target, caption, label, color) in [
            (
                PaletteTarget::Primary,
                "Color 1",
                "Pencil color",
                state.primary,
            ),
            (
                PaletteTarget::Secondary,
                "Color 2",
                "Fill color",
                state.secondary,
            ),
        ] {
            let action = target.edit();
            let selected = workspace.palette_target == target;
            let response = ui
                .add_enabled_ui(action.enabled(context), |ui| {
                    super::legacy_surface::button(
                        ui,
                        variant,
                        egui::Button::new("")
                            .min_size(Vec2::new(42.0, 68.0))
                            .selected(selected)
                            .frame_when_inactive(selected),
                        selected,
                    )
                })
                .inner;
            let rect = Rect::from_center_size(
                response.rect.center_top() + Vec2::new(0.0, 19.0),
                Vec2::splat(if target == PaletteTarget::Primary {
                    32.0
                } else {
                    26.0
                }),
            );
            ui.painter().rect_filled(rect, 0, Color32::WHITE);
            ui.painter().rect_filled(
                rect.shrink(3.0),
                0,
                Color32::from_rgb(color.0[0], color.0[1], color.0[2]),
            );
            ui.painter().rect_stroke(
                rect.shrink(0.5),
                0,
                ui.visuals().widgets.noninteractive.bg_stroke,
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                response.rect.center_bottom() - Vec2::new(0.0, 17.0),
                egui::Align2::CENTER_CENTER,
                tr(ui.ctx(), caption).replace(' ', "\n"),
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().text_color(),
            );
            let accessible = tr(ui.ctx(), label);
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::Role::ColorWell,
                    action.enabled(context) && ui.is_enabled(),
                    selected,
                    &accessible,
                )
            });
            let response = response.on_hover_text(accessible);
            if response.clicked() {
                workspace.palette_target = target;
            }
            if response.double_clicked() {
                actions.push(action);
            }
        }
        egui::Grid::new("classic-palette")
            .min_col_width(0.0)
            .min_row_height(20.0)
            .num_columns(10)
            .spacing(Vec2::splat(2.0))
            .show(ui, |ui| {
                let colors = PALETTE[..20]
                    .iter()
                    .map(|rgb| Some(Rgba([rgb[0], rgb[1], rgb[2], 255])))
                    .chain(workspace.custom_colors.iter().copied().take(10));
                for (index, color) in colors.enumerate() {
                    let slot = index.saturating_sub(20);
                    let rgb = color.map_or([255; 3], |color| [color.0[0], color.0[1], color.0[2]]);
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::click());
                    ui.painter().rect_filled(rect, 0, Color32::WHITE);
                    ui.painter().rect_filled(
                        rect.shrink(2.0),
                        0,
                        if color.is_some() {
                            Color32::from_rgb(rgb[0], rgb[1], rgb[2])
                        } else {
                            ui.visuals().panel_fill
                        },
                    );
                    ui.painter().rect_stroke(
                        rect.shrink(0.5),
                        0,
                        if response.hovered() {
                            ui.visuals().selection.stroke
                        } else {
                            ui.visuals().widgets.noninteractive.bg_stroke
                        },
                        egui::StrokeKind::Inside,
                    );
                    let label = if index < 20 {
                        super::palette::accessible(ui.ctx(), rgb)
                    } else {
                        tr_format(
                            ui.ctx(),
                            "Palette custom color { $number }",
                            &[("number", (slot + 1).to_string())],
                        )
                    };
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &label)
                    });
                    let tip = if index < 20 {
                        super::palette::tooltip(ui.ctx(), index, rgb)
                    } else {
                        tr_format(
                            ui.ctx(),
                            "Custom color { $number }",
                            &[("number", (slot + 1).to_string())],
                        )
                    };
                    let response = response.on_hover_text(tip);
                    if response.clicked() {
                        if let Some(color) = color {
                            workspace.palette_target.apply(state, color);
                        } else {
                            actions.push(workspace.palette_target.custom(slot as u8));
                        }
                    }
                    if response.double_clicked() && color.is_some() {
                        actions.push(if index < 20 {
                            workspace.palette_target.edit()
                        } else {
                            workspace.palette_target.custom(slot as u8)
                        });
                    }
                    if response.secondary_clicked() {
                        if let Some(color) = color {
                            state.secondary = color;
                        } else {
                            actions.push(PaletteTarget::Secondary.custom(slot as u8));
                        }
                    }
                    if index % 10 == 9 {
                        ui.end_row();
                    }
                }
            });
        if super::legacy_controls::large_button(
            ui,
            context,
            workspace.palette_target.edit(),
            Icon::EditColors,
            "Edit colors",
        )
        .clicked()
        {
            actions.push(workspace.palette_target.edit());
        }
    });
}
