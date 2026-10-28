//! A single real native cancellation/range control needs no client geometry.
use super::*;

pub(super) fn metadata(
    pointer: POINTER_INFO,
    message: Message,
    modifiers: paintcraft_input::Modifiers,
) -> Option<Info> {
    if pointer.historyCount != 1
        || (message != Message::Leave
            && pointer.pointerFlags.0 & (model::CANCELED | model::CAPTURE_CHANGED) == 0)
    {
        return None;
    }
    Some(Info {
        device: pointer.sourceDevice.0 as usize as u64,
        pointer: pointer.pointerId,
        frame: pointer.frameId,
        flags: pointer.pointerFlags.0,
        performance: pointer.PerformanceCount,
        ticks: pointer.dwTime,
        button_change: pointer.ButtonChangeType.0,
        // Actual SDK screen position is retained only in this private copy.
        // The checked Cancel/Leave model always emits position=None; this value
        // is never labeled/delivered as a client point without conversion.
        position: (
            pointer.ptPixelLocationRaw.x as f64,
            pointer.ptPixelLocationRaw.y as f64,
        ),
        // No fresh client permission is inferred from unavailable geometry.
        // An existing held contact retains its original client/native route.
        client: false,
        modifiers: paintcraft_input::Modifiers {
            shift: pointer.dwKeyStates & POINTER_MOD_SHIFT != 0,
            control: pointer.dwKeyStates & POINTER_MOD_CTRL != 0,
            ..modifiers
        },
        ..Info::default()
    })
}
