//! Actual PT_TOUCH history acquisition for the canonical owned touch contract.
//! No frame skipping: every pending sibling MSG remains available to the hook.
use super::*;

pub(super) fn acquire(
    target: &mut Target,
    id: u32,
    message: Message,
    message_flags: u32,
    native_chrome: bool,
) -> Result<bool> {
    let mut current = POINTER_TOUCH_INFO::default();
    // SAFETY: caller-owned bounded storage; no dispatch/message retrieval or
    // external callback occurs between these current-message query/fetch calls.
    unsafe { GetPointerTouchInfo(id, &mut current) }
        .map_err(|_| error("Touch metadata unavailable"))?;
    if !pointer_flags_match(current.pointerInfo.pointerFlags.0, message_flags) {
        return Err(error("Touch message flags disagree with native metadata"));
    }
    if current.pointerInfo.pointerId != id
        || current.pointerInfo.pointerType != PT_TOUCH
        || current.pointerInfo.hwndTarget != target.hwnd
    {
        return Err(error("Current touch source identity/target mismatch"));
    }
    if !target.emitter.report.device(crate::Device {
        id: current.pointerInfo.sourceDevice.0 as usize as u64,
        kind: paintcraft_input::tablet::DeviceKind::Finger,
        advertised: crate::Axes::default(),
        observed: crate::Axes::default(),
        native_capabilities: 0,
    }) {
        return Err(error("Native touch device admission refused"));
    }
    if let Some(metadata) = control::metadata(current.pointerInfo, message, mouse::modifiers()) {
        target.storage.copied.clear();
        target.storage.copied.push(metadata);
        return target
            .touch
            .history(
                &target.storage.copied,
                message,
                unsafe { GetTickCount64() },
                &mut target.storage.fingers,
            )
            .map_err(|_| error("Touch control metadata invalid"));
    }
    let mut count = 0;
    unsafe { GetPointerTouchInfoHistory(id, &mut count, None) }
        .map_err(|_| error("Touch history query failed"))?;
    let expected = count;
    if count == 0 || count as usize > LIMIT || count != current.pointerInfo.historyCount {
        return Err(error("Touch history admission refused"));
    }
    unsafe {
        GetPointerTouchInfoHistory(
            id,
            &mut count,
            Some(target.storage.touch_history.as_mut_ptr()),
        )
    }
    .map_err(|_| error("Touch history fetch failed"))?;
    if count != expected || count as usize > LIMIT || target.storage.touch_history[0] != current {
        return Err(error("Touch history changed or was truncated"));
    }
    let transformed = match unsafe {
        GetPointerInputTransform(id, &mut target.storage.transforms[..count as usize])
    } {
        Ok(()) => true,
        Err(error) if error.code() == windows::core::HRESULT::from_win32(ERROR_NO_DATA.0) => false,
        Err(_) => return Err(error("Touch transform unavailable")),
    };
    let mut rect = RECT::default();
    unsafe { GetClientRect(target.hwnd, &mut rect) }
        .map_err(|_| error("Touch client bounds unavailable"))?;
    let modifiers = mouse::modifiers();
    target.storage.copied.clear();
    for (index, touch) in target.storage.touch_history[..count as usize]
        .iter()
        .enumerate()
    {
        let pointer = touch.pointerInfo;
        if pointer.pointerType != PT_TOUCH || pointer.hwndTarget != target.hwnd {
            return Err(error("Touch history owner mismatch"));
        }
        if !target
            .barrier
            .admits(pointer.PerformanceCount, pointer.dwTime)
        {
            return Err(error("Touch history crosses a source boundary"));
        }
        if pointer.pointerFlags.0 & POINTER_FLAG_HASTRANSFORM.0 != 0 && !transformed {
            return Err(error("Touch transform flag has no transform"));
        }
        let mut point = pointer.ptPixelLocationRaw;
        let position = if transformed {
            super::super::transform::client(
                unsafe { target.storage.transforms[index].Anonymous.m },
                (point.x as f64, point.y as f64),
            )
            .ok_or_else(|| error("Touch transform is not invertible 2D"))?
        } else {
            if !unsafe { ScreenToClient(target.hwnd, &mut point) }.as_bool() {
                return Err(error("Touch client conversion failed"));
            }
            (point.x as f64, point.y as f64)
        };
        target.storage.copied.push(Info {
            device: pointer.sourceDevice.0 as usize as u64,
            pointer: pointer.pointerId,
            frame: pointer.frameId,
            button_change: pointer.ButtonChangeType.0,
            flags: pointer.pointerFlags.0,
            performance: pointer.PerformanceCount,
            ticks: pointer.dwTime,
            position,
            client: !native_chrome
                && position.0 >= rect.left as f64
                && position.0 < rect.right as f64
                && position.1 >= rect.top as f64
                && position.1 < rect.bottom as f64,
            modifiers: paintcraft_input::Modifiers {
                shift: pointer.dwKeyStates & POINTER_MOD_SHIFT != 0,
                control: pointer.dwKeyStates & POINTER_MOD_CTRL != 0,
                ..modifiers
            },
            ..Info::default()
        });
    }
    // Native touch pressure/contact values are deliberately not mapped to pen
    // axes. Finger lifecycle/coordinates use the explicit touch descriptor.
    target
        .touch
        .history(
            &target.storage.copied,
            message,
            unsafe { GetTickCount64() },
            &mut target.storage.fingers,
        )
        .map_err(|_| error("Touch history validation failed"))
}
