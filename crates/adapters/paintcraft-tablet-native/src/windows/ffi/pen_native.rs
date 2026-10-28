//! Actual SDK pen-history acquisition, separate from ownership and dispatch.
use super::*;

pub(super) fn acquire(
    target: &mut Target,
    id: u32,
    phase: Message,
    message_flags: u32,
    native_chrome: bool,
) -> Result<bool> {
    let mut current = POINTER_PEN_INFO::default();
    // SAFETY: initialized caller-owned output. Query and fetch remain in one
    // current MSG; no next-message retrieval, dispatch, or external callback.
    unsafe { GetPointerPenInfo(id, &mut current) }
        .map_err(|_| error("Pen metadata unavailable"))?;
    if !pointer_flags_match(current.pointerInfo.pointerFlags.0, message_flags) {
        return Err(error("Pen message flags disagree with native metadata"));
    }
    if current.pointerInfo.pointerId != id
        || current.pointerInfo.pointerType != PT_PEN
        || current.pointerInfo.hwndTarget != target.hwnd
    {
        return Err(error("Current pen source identity/target mismatch"));
    }
    let device = current.pointerInfo.sourceDevice.0 as usize as u64;
    let kind = if current.penFlags & (model::INVERTED | model::ERASER) != 0 {
        paintcraft_input::tablet::DeviceKind::Eraser
    } else {
        paintcraft_input::tablet::DeviceKind::Pen
    };
    // PenMask describes values in this packet, not hardware-advertised axes.
    // Report actual observations separately, without claiming absent axes.
    if !target.emitter.report.device(crate::Device {
        id: device,
        kind,
        advertised: crate::Axes::default(),
        observed: crate::Axes::default(),
        native_capabilities: 0,
    }) {
        return Err(error("Windows device report admission refused"));
    }
    if let Some(mut metadata) = control::metadata(current.pointerInfo, phase, mouse::modifiers()) {
        metadata.pen_flags = current.penFlags;
        metadata.mask = current.penMask;
        metadata.pressure = current.pressure;
        metadata.rotation = current.rotation;
        metadata.tilt = (current.tiltX, current.tiltY);
        target.storage.copied.clear();
        target.storage.copied.push(metadata);
        return publish(target, phase);
    }
    let mut count = 0;
    unsafe { GetPointerPenInfoHistory(id, &mut count, None) }
        .map_err(|_| error("Pen history query failed"))?;
    let expected = count;
    if expected == 0 || expected as usize > LIMIT || expected != current.pointerInfo.historyCount {
        return Err(error("Pen history admission refused"));
    }
    unsafe { GetPointerPenInfoHistory(id, &mut count, Some(target.storage.history.as_mut_ptr())) }
        .map_err(|_| error("Pen history fetch failed"))?;
    // Windows may succeed with a partial buffer and return the required total.
    if count != expected || count as usize > LIMIT || target.storage.history[0] != current {
        return Err(error("Pen history changed or was truncated"));
    }
    let transformed = match unsafe {
        GetPointerInputTransform(id, &mut target.storage.transforms[..count as usize])
    } {
        Ok(()) => true,
        Err(error) if error.code() == windows::core::HRESULT::from_win32(ERROR_NO_DATA.0) => false,
        Err(_) => return Err(error("Native pointer transform unavailable")),
    };
    target.storage.copied.clear();
    let modifiers = mouse::modifiers();
    let mut rect = RECT::default();
    unsafe { GetClientRect(target.hwnd, &mut rect) }
        .map_err(|_| error("Client bounds unavailable"))?;
    for (index, pen) in target.storage.history[..count as usize].iter().enumerate() {
        let p = pen.pointerInfo;
        if p.pointerFlags.0 & POINTER_FLAG_HASTRANSFORM.0 != 0 && !transformed {
            return Err(error("Native transform flag has no transform"));
        }
        if !target.barrier.admits(p.PerformanceCount, p.dwTime) {
            return Err(error(
                "Pen history crosses an unrepresentable source boundary",
            ));
        }
        if p.hwndTarget != target.hwnd || p.pointerType != PT_PEN {
            return Err(error("Pen history target mismatch"));
        }
        let mut point = p.ptPixelLocationRaw;
        let position = if transformed {
            // SAFETY: Windows initialized this corresponding union entry; any
            // floating bit pattern is valid and the checked inverse rejects NaN.
            super::super::transform::client(
                unsafe { target.storage.transforms[index].Anonymous.m },
                (point.x as f64, point.y as f64),
            )
            .ok_or_else(|| error("Native pointer transform is not invertible 2D"))?
        } else {
            if !unsafe { ScreenToClient(target.hwnd, &mut point) }.as_bool() {
                return Err(error("Pen client conversion failed"));
            }
            (point.x as f64, point.y as f64)
        };
        target.storage.copied.push(Info {
            device: p.sourceDevice.0 as usize as u64,
            pointer: p.pointerId,
            frame: p.frameId,
            button_change: p.ButtonChangeType.0,
            flags: p.pointerFlags.0,
            pen_flags: pen.penFlags,
            mask: pen.penMask,
            pressure: pen.pressure,
            rotation: pen.rotation,
            tilt: (pen.tiltX, pen.tiltY),
            performance: p.PerformanceCount,
            ticks: p.dwTime,
            position,
            client: !native_chrome
                && position.0 >= rect.left as f64
                && position.0 < rect.right as f64
                && position.1 >= rect.top as f64
                && position.1 < rect.bottom as f64,
            modifiers: paintcraft_input::Modifiers {
                shift: p.dwKeyStates & POINTER_MOD_SHIFT != 0,
                control: p.dwKeyStates & POINTER_MOD_CTRL != 0,
                ..modifiers
            },
        });
    }
    publish(target, phase)
}
fn publish(target: &mut Target, phase: Message) -> Result<bool> {
    target
        .state
        .history(
            &target.storage.copied,
            phase,
            unsafe { GetTickCount64() },
            &mut target.storage.output,
        )
        .map_err(|_| error("Pen history validation failed"))?;
    for output in &target.storage.output {
        if let Output::Packet(packet) = output {
            target.storage.events.push(CaptureEvent::Tablet(*packet));
        }
    }
    for (identity, time) in target.state.retired().into_iter().flatten() {
        target
            .storage
            .events
            .push(CaptureEvent::IdentityRetired { identity, time });
    }
    Ok(target
        .storage
        .output
        .iter()
        .any(|p| !matches!(p, Output::Native)))
}
