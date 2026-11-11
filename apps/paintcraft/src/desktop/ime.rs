//! The real owned loop restores only facility boundaries omitted by egui-winit.
use super::*;
use std::collections::TryReserveError;
// These deprecated egui DTOs are intentionally delivered by OUR maintained
// integration, not claimed to be emitted by egui-winit or other native shells.
#[expect(
    deprecated,
    reason = "preserve actual winit IME facility boundaries in the owned loop"
)]
fn boundary(event: &WindowEvent) -> Option<egui::Event> {
    match event {
        WindowEvent::Ime(winit::event::Ime::Enabled) => {
            Some(egui::Event::Ime(egui::ImeEvent::Enabled))
        }
        WindowEvent::Ime(winit::event::Ime::Disabled) => {
            Some(egui::Event::Ime(egui::ImeEvent::Disabled))
        }
        _ => None,
    }
}
pub(super) fn append(
    raw: &mut egui::RawInput,
    event: &WindowEvent,
) -> std::result::Result<bool, TryReserveError> {
    let Some(event) = boundary(event) else {
        return Ok(false);
    };
    raw.events.try_reserve(1)?;
    raw.events.push(event);
    Ok(true)
}
pub(super) fn retire_failed(host: &mut Host, raw: &mut egui::RawInput, error: &TryReserveError) {
    raw.events.clear();
    host.retire();
    let app = host.app_mut();
    app.revoke_canvas_ime();
    app.editor.set_status(format!("Input retired: {error}"));
}
