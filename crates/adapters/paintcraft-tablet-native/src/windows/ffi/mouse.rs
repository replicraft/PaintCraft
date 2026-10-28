//! Actual retrieved mouse MSGs. No coordinate/time matching against pen data.
use super::*;
use crate::{MouseKind, MouseRecord, WheelPhase, WindowPosition};
use paintcraft_input::Modifiers;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;

pub(super) fn modifiers() -> Modifiers {
    // SAFETY: GetKeyState describes this thread's retrieved native queue state.
    unsafe {
        Modifiers {
            shift: GetKeyState(VK_SHIFT.0 as i32) < 0,
            control: GetKeyState(VK_CONTROL.0 as i32) < 0,
            alt: GetKeyState(VK_MENU.0 as i32) < 0,
            command: GetKeyState(VK_LWIN.0 as i32) < 0 || GetKeyState(VK_RWIN.0 as i32) < 0,
        }
    }
}
pub(super) unsafe fn held() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2]
        .iter()
        .any(|v| unsafe { GetAsyncKeyState(v.0 as i32) } < 0)
}
pub(super) fn observe(target: &mut Target, message: &NativeMessage) -> Result<bool> {
    let Some(kind) = classify(message.message, message.w_param.0)? else {
        return Ok(false);
    };
    // SAFETY: extra-info is queried before translating this retrieved MSG.
    let extra = unsafe { GetMessageExtraInfo() }.0 as usize;
    // The documented bit 0x80 distinguishes touch promotion. Canonical pen and touch replace their promoted pointer streams.
    if model::promoted(extra) {
        return Ok(true);
    }
    // MSG.pt is the actual full-width cursor position when this message was
    // posted. It also supplies WM_MOUSELEAVE, whose lParam is explicitly zero.
    // Never use receipt-time GetCursorPos or truncate to lParam's signed16bits.
    let mut point = message
        .screen
        .ok_or_else(|| error("Retrieved mouse position required"))?;
    if !unsafe { ScreenToClient(target.hwnd, &mut point) }.as_bool() {
        return Err(error("Mouse client conversion failed"));
    }
    let mut rect = RECT::default();
    unsafe { GetClientRect(target.hwnd, &mut rect) }
        .map_err(|_| error("Mouse client bounds unavailable"))?;
    let time = super::super::clock::ticks(
        message
            .ticks
            .ok_or_else(|| error("Retrieved mouse clock required"))?,
        unsafe { GetTickCount64() },
        target.state.epoch,
    )
    .ok_or_else(|| error("Mouse native clock invalid"))?;
    target.storage.events.push(CaptureEvent::Mouse(MouseRecord {
        kind,
        position: WindowPosition::PhysicalPixels(point.x as f64, point.y as f64),
        client: point.x >= rect.left
            && point.x < rect.right
            && point.y >= rect.top
            && point.y < rect.bottom,
        modifiers: modifiers(),
        time,
    }));
    Ok(false)
}

pub(super) fn classify(message: u32, wparam: usize) -> Result<Option<MouseKind>> {
    let kind = match message {
        WM_MOUSEMOVE => MouseKind::Motion,
        WM_MOUSELEAVE => MouseKind::Leave,
        WM_LBUTTONDOWN | WM_LBUTTONUP => MouseKind::Button {
            number: 0,
            pressed: message == WM_LBUTTONDOWN,
        },
        WM_RBUTTONDOWN | WM_RBUTTONUP => MouseKind::Button {
            number: 1,
            pressed: message == WM_RBUTTONDOWN,
        },
        WM_MBUTTONDOWN | WM_MBUTTONUP => MouseKind::Button {
            number: 2,
            pressed: message == WM_MBUTTONDOWN,
        },
        WM_XBUTTONDOWN | WM_XBUTTONUP => {
            let button = (wparam >> 16) & 0xffff;
            if !matches!(button, 1 | 2) {
                return Err(error("Unknown actual X button"));
            }
            MouseKind::Button {
                number: button as u8 + 2,
                pressed: message == WM_XBUTTONDOWN,
            }
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let delta = (wparam >> 16) as u16 as i16 as f64 / 120.0;
            MouseKind::Wheel {
                delta: if message == WM_MOUSEWHEEL {
                    (0.0, delta)
                } else {
                    // WM_MOUSEHWHEEL positive means wheel right. Our shared
                    // GUI delta follows winit: positive X moves content right.
                    (-delta, 0.0)
                },
                precise: false,
                physical_scale: 1.0,
                phase: WheelPhase::Move,
                momentum: false,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(kind))
}
