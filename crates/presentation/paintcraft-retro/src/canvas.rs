//! Scrollable historical canvas presentation with caller-owned image/input callbacks.
use egui::{Context, Id, Ui, Vec2};

/// Queue a view-only pan for the exact caller-supplied document canvas owner.
pub fn pan_by(ctx: &Context, id: Id, delta: Vec2) {
    if !delta.is_finite() {
        return;
    }
    ctx.data_mut(|data| {
        let key = id.with("retro-pan-request");
        let old = data.get_temp::<Vec2>(key).unwrap_or_default();
        let total = old + delta;
        if total.is_finite() {
            data.insert_temp(key, total);
        }
    });
    ctx.request_repaint();
}

/// Paint the actual host canvas in a separately scoped, scrollable viewport.
/// `id` remains stable for its document; `scope` identifies current input/zoom geometry.
pub fn canvas<R>(
    ui: &mut Ui,
    id: Id,
    scope: Id,
    size: Vec2,
    paint: impl FnOnce(&mut Ui, egui::Response, egui::Painter) -> R,
) -> R {
    let viewport = ui.available_rect_before_wrap();
    let layout_key = id.with("retro-anchor-layout");
    let predicted = ui
        .data(|data| data.get_temp::<(Vec2, Vec2)>(layout_key))
        .map_or(viewport.size(), |(outer, inner)| {
            viewport.size() - (outer - inner)
        })
        .max(Vec2::splat(1.0));
    let pending = ui.data_mut(|data| data.remove_temp::<Vec2>(id.with("retro-pan-request")));
    let anchor_key = id.with("retro-anchor-request");
    let anchor = ui
        .data_mut(|data| {
            let anchor = data.get_temp::<crate::anchor::Anchor>(anchor_key);
            data.remove::<crate::anchor::Anchor>(anchor_key);
            anchor
        })
        .filter(|anchor| anchor.scope == scope && pending.is_none());
    let mut area = egui::ScrollArea::both()
        .id_salt(("retro-canvas", id))
        .auto_shrink([false, false]);
    if let Some(delta) = pending {
        let old = ui
            .data(|data| data.get_temp::<Vec2>(id.with("retro-pan-offset")))
            .unwrap_or_default();
        area = area.scroll_offset((old - delta).max(Vec2::ZERO));
    }
    let mut anchored = None;
    if let Some(anchor) = anchor {
        let maximum = (Vec2::splat(3.0) + size - predicted).max(Vec2::ZERO);
        let offset = anchor.offset(viewport.min, Vec2::splat(3.0), maximum);
        area = area.scroll_offset(offset);
        anchored = Some(offset);
    }
    let kinetic = ui.spacing().scroll.kinetic;
    if anchor.is_some() {
        ui.spacing_mut().scroll.kinetic.rubber_band = false;
        ui.spacing_mut().scroll.kinetic.decay_time = 0.0;
    }
    let output = area.show_viewport(ui, |ui, view| {
        if anchor.is_some() {
            ui.scroll_to_rect_animation(
                egui::Rect::from_center_size(viewport.min + view.size() * 0.5, Vec2::ZERO),
                Some(egui::Align::Center),
                egui::style::ScrollAnimation::none(),
            );
        }
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.add_space(3.0);
            ui.push_id(scope, |ui| {
                let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
                paint(ui, response, painter)
            })
            .inner
        })
        .inner
    });
    if anchor.is_some() {
        ui.spacing_mut().scroll.kinetic = kinetic;
    }
    let actual = output.inner_rect.size();
    ui.data_mut(|data| data.insert_temp(layout_key, (viewport.size(), actual)));
    let agrees = anchored
        .is_some_and(|offset| (output.state.unclamped_offset() - offset).length() < 1.0)
        && output.state.velocity() == Vec2::ZERO;
    if let Some(request) = anchor.and_then(|request| {
        request.settle(predicted, actual, ui.ctx().cumulative_frame_nr(), agrees)
    }) {
        ui.data_mut(|data| {
            if data.get_temp::<crate::anchor::Anchor>(anchor_key).is_none()
                && data
                    .get_temp::<Vec2>(id.with("retro-pan-request"))
                    .is_none()
            {
                data.insert_temp(anchor_key, request);
            }
        });
        ui.ctx().request_repaint();
    }
    ui.data_mut(|data| {
        data.insert_temp(id.with("retro-pan-offset"), output.state.clamped_offset())
    });
    output.inner
}
