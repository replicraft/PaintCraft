//! Complete egui drag sliders with coalesced taps and consumed arrow events.
use egui::{Key, PointerButton, Response, Ui};

pub(super) fn finish(ui: &Ui, original: Response, vertical: bool) -> (Response, Option<f32>) {
    let changed = original.changed();
    let mut response = original.interact(egui::Sense::click_and_drag());
    if changed {
        response.mark_changed();
    }
    // Preserve egui's smart aim for ordinary presses/drags. Only complete a
    // primary press and release delivered together, which drag sense misses.
    let coalesced = ui.input(|input| {
        input.pointer.button_pressed(PointerButton::Primary)
            && input.pointer.button_released(PointerButton::Primary)
    });
    let tap = if coalesced && response.clicked() {
        response.interact_pointer_pos().map(|point| {
            response.request_focus();
            let rect = response.rect;
            if vertical {
                let travel = rect.y_range().shrink(rect.width() / 2.5);
                (1.0 - (point.y - travel.min) / travel.span()).clamp(0.0, 1.0)
            } else {
                let travel = rect.x_range().shrink(rect.height() / 2.5);
                ((point.x - travel.min) / travel.span()).clamp(0.0, 1.0)
            }
        })
    } else {
        None
    };
    if response.has_focus() {
        ui.input_mut(|input| {
            let modifiers = input.modifiers;
            let keys = if vertical {
                [Key::ArrowUp, Key::ArrowDown]
            } else {
                [Key::ArrowLeft, Key::ArrowRight]
            };
            for key in keys {
                input.consume_key(modifiers, key);
            }
        });
    }
    (response, tap)
}

pub(super) fn changed_tap(ui: &Ui, response: &mut Response) {
    response.mark_changed();
    // egui painted before the missing tap was completed. Relayout uses the new
    // value for its shell as well as the decoration, within the pass budget.
    ui.ctx().request_discard("Classic slider coalesced tap");
    ui.ctx().request_repaint();
}
