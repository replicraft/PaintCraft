//! Sent-message lifecycle observation; MSG hooks alone cannot see these edges.
use super::*;
use windows::Win32::UI::Shell::DefSubclassProc;

pub(super) unsafe extern "system" fn subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: stable boxed Context registered by Hook::install. Clone before any
    // external Sink: dropping the guard there may retire the original box.
    let (inner, current) = {
        let context = unsafe { &*(data as *const Context) };
        (context.inner.clone(), context.current())
    };
    // Stale retained callbacks cannot acquire a later installation's authority,
    // even if the same HWND is reused. They retain only the normal native path.
    if !current {
        return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    }
    // Retain the original HWND owner through DefSubclassProc even when Sink
    // drops Installation during this callback. A nested borrow is handled by
    // the outer acquisition's retained Target rather than panicking in FFI.
    let _window = inner
        .target
        .try_borrow()
        .ok()
        .and_then(|t| t.as_ref().map(|t| t._window.clone()));
    if inner.hwnd.get() == hwnd.0 as usize && native_ui::recognizes(message) {
        return unsafe { native_ui::dispatch(&inner, hwnd, message, wparam, lparam) };
    }
    if matches!(
        message,
        WM_POINTERENTER
            | WM_POINTERLEAVE
            | WM_POINTERDOWN
            | WM_POINTERUP
            | WM_POINTERUPDATE
            | WM_NCPOINTERDOWN
            | WM_NCPOINTERUP
            | WM_NCPOINTERUPDATE
    ) {
        let actual = NativeMessage {
            hwnd,
            message,
            w_param: wparam,
            l_param: lparam,
            ticks: None,
            screen: None,
        };
        if ingress(&inner, &actual) {
            return LRESULT(0);
        }
    }
    let relevant = matches!(
        message,
        WM_SETFOCUS
            | WM_KILLFOCUS
            | WM_CANCELMODE
            | WM_CAPTURECHANGED
            | WM_POINTERCAPTURECHANGED
            | WM_NCDESTROY
            | WM_DPICHANGED
            | WM_MOVE
            | WM_SIZE
            | WM_POINTERDEVICECHANGE
    );
    if relevant && inner.processing.replace(true) {
        inner.reentered.set(true);
        return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let boundary = match message {
            WM_KILLFOCUS | WM_CANCELMODE | WM_CAPTURECHANGED | WM_POINTERCAPTURECHANGED => {
                Some(Boundary::FocusLost)
            }
            WM_SETFOCUS => {
                let event = {
                    let mut target = inner.target.borrow_mut();
                    target
                        .as_mut()
                        .and_then(|t| t.mouse_idle().map(|event| (t.emitter.clone(), event)))
                };
                if let Some((emitter, event)) = event {
                    super::super::batch::deliver(&emitter, &[event], false, || {
                        !inner.reentered.replace(false) && inner.active.get()
                    });
                }
                None
            }
            WM_NCDESTROY if inner.hwnd.get() == hwnd.0 as usize => Some(Boundary::WindowClosed),
            WM_POINTERDEVICECHANGE if wparam.0 as u32 == PDC_REMOVAL => {
                let emitter = {
                    let mut target = inner.target.borrow_mut();
                    let Some(target) = target.as_mut() else {
                        return;
                    };
                    target.fence();
                    target.emitter.clone()
                };
                // The supported page does not define a per-pointer lifetime
                // or button/clock payload. Do not infer an identity from lParam.
                // A removal notification is not a per-pointer timed Leave.
                // Fail the handoff so the host discards all unpublished input.
                emitter.source_failure();
                None
            }
            WM_POINTERDEVICECHANGE if wparam.0 as u32 == PDC_ARRIVAL => None,
            WM_DPICHANGED | WM_MOVE | WM_SIZE | WM_POINTERDEVICECHANGE => {
                Some(Boundary::ViewInvalidated)
            }
            _ => None,
        };
        if let Some(kind) = boundary {
            let (emitter, _window, clock_failed) = {
                let mut target = inner.target.borrow_mut();
                let Some(target) = target.as_mut() else {
                    return;
                };
                target.fence();
                if kind == Boundary::FocusLost {
                    target.mouse_recovery = true;
                }
                let mut clock_failed = false;
                if kind == Boundary::ViewInvalidated {
                    match Barrier::observe() {
                        Ok(barrier) => target.barrier = barrier,
                        Err(_) => clock_failed = true,
                    }
                }
                (target.emitter.clone(), target._window.clone(), clock_failed)
            };
            if clock_failed {
                emitter.source_failure();
                return;
            }
            if kind == Boundary::WindowClosed {
                // The native control and sealed retirement share one gate;
                // no concurrent prepared publication can slip between them.
                super::super::batch::close(&emitter);
                inner.active.set(false);
                emitter.report.installed(false);
                // Refdata remains owned solely by Installation. Never free it
                // here: this callback is still executing before DefSubclassProc.
            } else {
                super::super::batch::deliver(
                    &emitter,
                    &[CaptureEvent::Boundary { kind, time: None }],
                    false,
                    || !inner.reentered.replace(false) && inner.active.get(),
                );
            }
        }
    }));
    if relevant {
        inner.processing.set(false);
    }
    let failed = result
        .is_err()
        .then(|| inner.target.borrow().as_ref().map(|t| t.emitter.clone()))
        .flatten();
    if let Some(emitter) = failed {
        emitter.source_failure();
    }
    // Framework delivery remains unchanged; no current-scope/window-event join.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
