//! Classic edge rails retain egui scrolling and thumb interaction.
use egui::{Color32, Id, Pos2, Rect, Response, Sense, Shape, Ui, Vec2};

const RAIL: f32 = 14.0;
const STEP: f32 = 40.0;

pub(super) fn show<R>(ui: &mut Ui, id: Id, paint: impl FnOnce(&mut Ui) -> R) -> R {
    let viewport = ui.available_rect_before_wrap();
    let geometry_id = ui.scope_id().with(("canvas-scroll-inner", id));
    let pending_id = ui.scope_id().with(("canvas-scroll-request", id));
    let previous = ui
        .data(|data| data.get_temp::<Rect>(geometry_id))
        .unwrap_or(viewport);
    let tracks = Rect::from_min_max(
        (previous.min + Vec2::splat(RAIL)).min(previous.center()),
        (previous.max - Vec2::splat(RAIL)).max(previous.center()),
    );
    let scroll = &mut ui.spacing_mut().scroll;
    scroll.fade.strength = 0.0;
    scroll.floating = false;
    scroll.bar_width = 6.0;
    scroll.bar_inner_margin = 4.0;
    scroll.bar_outer_margin = 4.0;
    scroll.foreground_color = true;
    let background = ui.painter().add(Shape::Noop);
    let mut area = egui::ScrollArea::both()
        .id_salt(("classic-canvas", id))
        .scroll_bar_rect(tracks)
        .auto_shrink([false, false]);
    if let Some(offset) = ui.data_mut(|data| data.remove_temp::<Vec2>(pending_id)) {
        area = area.scroll_offset(offset);
    }
    let output = area.show(ui, paint);
    let inner = output.inner_rect;
    if inner != previous {
        ui.data_mut(|data| data.insert_temp(geometry_id, inner));
        ui.ctx()
            .request_discard("Classic canvas scrollbar geometry changed");
    }
    let fill = Color32::from_gray(if ui.visuals().dark_mode { 43 } else { 240 });
    let mut rails = Vec::new();
    let mut offset = output.state.clamped_offset();
    let maximum = (output.content_size - inner.size()).max(Vec2::ZERO);
    let mut changed = false;
    for (axis, title, labels) in [
        (
            0,
            "Horizontal canvas scrollbar",
            ["Scroll canvas left", "Scroll canvas right"],
        ),
        (
            1,
            "Vertical canvas scrollbar",
            ["Scroll canvas up", "Scroll canvas down"],
        ),
    ] {
        if maximum[axis] <= 0.0 {
            continue;
        }
        let rail = if axis == 0 {
            Rect::from_min_max(
                Pos2::new(inner.left(), inner.bottom()),
                Pos2::new(inner.right(), viewport.bottom()),
            )
        } else {
            Rect::from_min_max(
                Pos2::new(inner.right(), inner.top()),
                Pos2::new(viewport.right(), inner.bottom()),
            )
        };
        rails.push(Shape::rect_filled(rail, 0, fill));
        if let Some(response) = ui.ctx().read_response(output.id.with(axis)) {
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::ScrollBar, ui.is_enabled(), title)
            });
        }
        for (end, label) in labels.into_iter().enumerate() {
            let mut rect = rail;
            if end == 0 {
                rect.max[axis] = (rect.min[axis] + RAIL).min(rect.max[axis]);
            } else {
                rect.min[axis] = (rect.max[axis] - RAIL).max(rect.min[axis]);
            }
            let enabled = if end == 0 {
                offset[axis] > 0.0
            } else {
                offset[axis] < maximum[axis]
            };
            let response = arrow(
                ui,
                output.id.with((axis, end)),
                rect,
                axis,
                end,
                label,
                enabled,
            );
            if activate(ui, &response) {
                offset[axis] =
                    (offset[axis] + if end == 0 { -STEP } else { STEP }).clamp(0.0, maximum[axis]);
                changed = true;
            }
        }
    }
    ui.painter().set(background, Shape::Vec(rails));
    if changed {
        ui.data_mut(|data| data.insert_temp(pending_id, offset));
        ui.ctx()
            .request_discard("Classic canvas scroll arrow changed the view");
        ui.ctx().request_repaint();
    }
    output.inner
}

fn arrow(
    ui: &mut Ui,
    id: Id,
    rect: Rect,
    axis: usize,
    end: usize,
    label: &str,
    enabled: bool,
) -> Response {
    let mut builder = egui::UiBuilder::new()
        .scope_id(id.with("arrow-ui"))
        .max_rect(rect);
    if !enabled {
        builder = builder.disabled();
    }
    let child = ui.new_child(builder);
    let response = child.interact(rect, id, Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, child.is_enabled(), label));
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    if response.hovered() || response.is_pointer_button_down_on() || response.has_focus() {
        painter.rect_filled(rect, 2, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let ink = if child.is_enabled() {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let sign = if end == 0 { -1.0 } else { 1.0 };
    let center = rect.center();
    let along = if axis == 0 { Vec2::X } else { Vec2::Y };
    let across = if axis == 0 { Vec2::Y } else { Vec2::X };
    painter.add(Shape::convex_polygon(
        vec![
            center + along * sign * 3.0,
            center - along * sign * 2.0 + across * 3.0,
            center - along * sign * 2.0 - across * 3.0,
        ],
        ink,
        egui::Stroke::NONE,
    ));
    response.on_hover_text(label).on_disabled_hover_text(label)
}

/// Press immediately, then repeat while held. A release must not add a second step.
fn activate(ui: &Ui, response: &Response) -> bool {
    if ui.ctx().current_pass_index() != 0 {
        return false;
    }
    let id = response.id.with("scroll-arrow-repeat");
    let next = ui.data(|data| data.get_temp::<f64>(id));
    let now = ui.input(|input| input.time);
    let held = response.is_pointer_button_down_on()
        && ui.input(|input| {
            input
                .pointer
                .interact_pos()
                .is_some_and(|point| response.rect.contains(point))
        });
    let activate = if held {
        next.is_none_or(|deadline| now >= deadline)
    } else {
        response.clicked() && next.is_none()
    };
    if held {
        let deadline = if activate {
            now + if next.is_none() { 0.4 } else { 0.05 }
        } else {
            next.expect("held repeat deadline")
        };
        ui.data_mut(|data| data.insert_temp(id, deadline));
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(
                (deadline - now).max(0.0),
            ));
    } else {
        ui.data_mut(|data| data.remove::<f64>(id));
    }
    activate
}
