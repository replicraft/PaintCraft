//! Studio color controls edit session preferences, never document pixels.
use crate::Workspace;
use egui::{Color32, Sense, Vec2, ecolor::Hsva};
use paintcraft_core::types::Rgba;
use paintcraft_ui::chrome::{Controls, color_edit};
use paintcraft_ui::i18n::tr;

pub(super) fn controls(ui: &mut egui::Ui, state: &mut Controls, workspace: &mut Workspace) {
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut workspace.edit_secondary,
            false,
            tr(ui.ctx(), "Primary"),
        );
        color_edit(ui, "Primary color", &mut state.primary);
        ui.selectable_value(
            &mut workspace.edit_secondary,
            true,
            tr(ui.ctx(), "Secondary"),
        );
        color_edit(ui, "Secondary color", &mut state.secondary);
    });
    ui.horizontal(|ui| {
        if ui.button(tr(ui.ctx(), "Swap colors")).clicked() && ui.ctx().current_pass_index() == 0 {
            std::mem::swap(&mut state.primary, &mut state.secondary);
        }
        let label = if workspace.more_colors {
            "Less <<"
        } else {
            "More >>"
        };
        if ui.button(tr(ui.ctx(), label)).clicked() && ui.ctx().current_pass_index() == 0 {
            workspace.more_colors = !workspace.more_colors;
        }
    });
    let selected = if workspace.edit_secondary {
        state.secondary
    } else {
        state.primary
    };
    if let Some((color, secondary)) =
        wheel(ui, selected, workspace.edit_secondary, state.secondary.0[3])
    {
        workspace.edit_secondary = secondary;
        if secondary {
            state.secondary = color;
        } else {
            state.primary = color;
        }
    }
    palette(ui, state, workspace);
    if workspace.more_colors {
        ui.separator();
        let value = if workspace.edit_secondary {
            &mut state.secondary
        } else {
            &mut state.primary
        };
        ui.push_id(workspace.edit_secondary, |ui| advanced(ui, value));
    }
}

fn hsva(color: Rgba) -> Hsva {
    // An opaque conversion keeps RGB meaningful even when the chosen alpha is 0.
    Hsva::from_srgba_unmultiplied([color.0[0], color.0[1], color.0[2], 255])
}

fn wheel_color(delta: Vec2, radius: f32, alpha: u8) -> Rgba {
    let mut color = Hsva {
        h: delta.y.atan2(delta.x).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU,
        s: (delta.length() / radius).clamp(0.0, 1.0),
        v: 1.0,
        a: 1.0,
    }
    .to_srgba_unmultiplied();
    color[3] = alpha;
    Rgba(color)
}

fn wheel(
    ui: &mut egui::Ui,
    selected: Rgba,
    secondary: bool,
    secondary_alpha: u8,
) -> Option<(Rgba, bool)> {
    const RADIUS: f32 = 86.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::splat(2.0 * RADIUS + 8.0), Sense::click_and_drag());
    let accessible = tr(ui.ctx(), "Color wheel");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::ColorWell, ui.is_enabled(), &accessible)
    });
    let center = rect.center();
    let mut mesh = egui::epaint::Mesh::default();
    mesh.colored_vertex(center, Color32::WHITE);
    const SEGMENTS: u32 = 96;
    const RINGS: u32 = 12;
    for ring in 1..=RINGS {
        for i in 0..=SEGMENTS {
            let angle = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let delta = Vec2::angled(angle) * (RADIUS * ring as f32 / RINGS as f32);
            let rgba = wheel_color(delta, RADIUS, 255).0;
            mesh.colored_vertex(center + delta, Color32::from_rgb(rgba[0], rgba[1], rgba[2]));
            if i > 0 {
                let outer = 1 + (ring - 1) * (SEGMENTS + 1) + i;
                if ring == 1 {
                    mesh.add_triangle(0, outer - 1, outer);
                } else {
                    let inner = outer - SEGMENTS - 1;
                    mesh.add_triangle(inner - 1, outer - 1, outer);
                    mesh.add_triangle(inner - 1, outer, inner);
                }
            }
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    let hsv = hsva(selected);
    let marker = center + Vec2::angled(hsv.h * std::f32::consts::TAU) * (hsv.s * RADIUS);
    ui.painter()
        .circle_stroke(marker, 4.0, egui::Stroke::new(2.0, Color32::BLACK));
    ui.painter()
        .circle_stroke(marker, 3.0, egui::Stroke::new(1.0, Color32::WHITE));
    let mut result = None;
    if (response.clicked() || response.secondary_clicked() || response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
    {
        let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
        if origin.distance(center) <= RADIUS {
            let right =
                response.secondary_clicked() || response.dragged_by(egui::PointerButton::Secondary);
            let target = secondary || right;
            let alpha = if target {
                secondary_alpha
            } else {
                selected.0[3]
            };
            result = Some((wheel_color(pos - center, RADIUS, alpha), target));
            response.request_focus();
        }
    }
    if response.has_focus() && ui.ctx().current_pass_index() == 0 {
        let (h, s) = ui.input_mut(|i| {
            let h = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) as i32
                - i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) as i32;
            let s = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) as i32
                - i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) as i32;
            (h, s)
        });
        if h != 0 || s != 0 {
            let angle = (hsv.h + h as f32 / 360.0) * std::f32::consts::TAU;
            let delta = Vec2::angled(angle) * ((hsv.s + s as f32 / 100.0).clamp(0.0, 1.0) * RADIUS);
            result = Some((wheel_color(delta, RADIUS, selected.0[3]), secondary));
        }
    }
    response.on_hover_text(tr(ui.ctx(), "Choose hue and saturation. Right-click edits secondary. Arrow keys change hue and saturation."));
    result
}

#[derive(Clone)]
struct HexDraft {
    text: String,
    source: Rgba,
}

fn advanced(ui: &mut egui::Ui, color: &mut Rgba) {
    for (index, label) in ["Red", "Green", "Blue", "Alpha"].into_iter().enumerate() {
        ui.add(egui::Slider::new(&mut color.0[index], 0..=255).text(tr(ui.ctx(), label)));
    }
    let id = ui.scope_id().with("hex-rgba");
    let mut draft = ui
        .data_mut(|d| d.get_temp::<HexDraft>(id))
        .unwrap_or_else(|| HexDraft {
            text: color.to_hex(),
            source: *color,
        });
    if draft.source != *color {
        draft = HexDraft {
            text: color.to_hex(),
            source: *color,
        };
    }
    ui.horizontal(|ui| {
        let label = ui.label(tr(ui.ctx(), "Hex RGBA"));
        let response = ui
            .add(
                egui::TextEdit::singleline(&mut draft.text)
                    .id(id)
                    .desired_width(110.0),
            )
            .labelled_by(label.id);
        let valid = Rgba::parse(&draft.text);
        if response.changed()
            && let Some(value) = valid
        {
            *color = value;
            draft.source = value;
        }
        if response.lost_focus() && valid.is_none() {
            draft.text = color.to_hex();
        }
    });
    if Rgba::parse(&draft.text).is_none() {
        ui.weak(tr(
            ui.ctx(),
            "Use #RRGGBBAA; incomplete text has not changed the color.",
        ));
    }
    ui.data_mut(|d| d.insert_temp(id, draft));
}

fn palette(ui: &mut egui::Ui, state: &mut Controls, workspace: &mut Workspace) {
    egui::Grid::new("studio-palette")
        .num_columns(8)
        .spacing(Vec2::splat(2.0))
        .show(ui, |ui| {
            for (i, rgb) in PALETTE.into_iter().enumerate() {
                let color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                let label = format!("Color #{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
                let r = ui.add(
                    egui::Button::new("")
                        .fill(color)
                        .min_size(Vec2::splat(20.0)),
                );
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &label)
                });
                if r.clicked() || r.secondary_clicked() {
                    workspace.edit_secondary |= r.secondary_clicked();
                    let target = if workspace.edit_secondary {
                        &mut state.secondary
                    } else {
                        &mut state.primary
                    };
                    *target = Rgba([rgb[0], rgb[1], rgb[2], 255]);
                }
                if i % 8 == 7 {
                    ui.end_row();
                }
            }
        });
}

const PALETTE: [[u8; 3]; 16] = [
    [0, 0, 0],
    [127, 127, 127],
    [128, 0, 0],
    [255, 0, 0],
    [255, 128, 0],
    [255, 255, 0],
    [0, 128, 0],
    [0, 128, 255],
    [255, 255, 255],
    [192, 192, 192],
    [128, 0, 128],
    [255, 128, 192],
    [128, 64, 0],
    [128, 255, 128],
    [0, 255, 255],
    [0, 0, 128],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_cardinal_hues_center_clamp_and_zero_alpha_keep_straight_rgb() {
        assert_eq!(wheel_color(Vec2::ZERO, 1.0, 0), Rgba([255, 255, 255, 0]));
        assert_eq!(
            wheel_color(Vec2::new(1.0, 0.0), 1.0, 37),
            Rgba([255, 0, 0, 37])
        );
        assert_eq!(
            wheel_color(Vec2::new(5.0, 0.0), 1.0, 37),
            Rgba([255, 0, 0, 37])
        );
        let green = wheel_color(Vec2::angled(std::f32::consts::TAU / 3.0), 1.0, 255).0;
        assert!(green[0] <= 1 && green[1] == 255 && green[2] <= 1);
        let blue = wheel_color(Vec2::angled(-std::f32::consts::TAU / 3.0), 1.0, 255).0;
        assert!(blue[0] <= 1 && blue[1] <= 1 && blue[2] == 255);
        assert!(hsva(Rgba([255, 0, 0, 0])).s > 0.99);
    }
}
