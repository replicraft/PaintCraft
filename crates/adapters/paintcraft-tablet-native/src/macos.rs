//! Main-thread local monitor; no global monitoring or accessibility permission.
use super::*;
use block2::RcBlock;
use objc2::{
    MainThreadMarker,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_app_kit::{
    NSApplicationDidResignActiveNotification, NSEvent, NSEventMask, NSEventModifierFlags,
    NSEventPhase, NSEventSubtype, NSEventType, NSPointingDeviceType, NSView, NSWindow,
    NSWindowDidBecomeKeyNotification, NSWindowDidResignKeyNotification,
    NSWindowWillCloseNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObjectProtocol};
use paintcraft_input::{Modifiers, tablet::DeviceKind};
use std::{
    ptr::NonNull,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
mod state;
use state::{Action, Point, State};
static EPOCH: AtomicU64 = AtomicU64::new(1);
pub(super) struct Monitor {
    token: Retained<AnyObject>,
    observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    center: Retained<NSNotificationCenter>,
    _window: Retained<NSWindow>,
    state: Arc<Mutex<State>>,
    closed: Arc<AtomicBool>,
    emitter: Emitter,
}
impl Monitor {
    pub(super) fn new(
        handle: raw_window_handle::AppKitWindowHandle,
        emitter: Emitter,
    ) -> Result<Self> {
        let mtm = MainThreadMarker::new().ok_or_else(|| {
            Error::new(
                ErrorCode::InputSchema,
                "Tablet monitor requires the main thread",
            )
        })?;
        // SAFETY: HasWindowHandle supplied a live NSView for the duration of
        // installation. Retaining it and its window owns subsequent Cocoa use.
        let view = unsafe { Retained::retain(handle.ns_view.cast::<NSView>().as_ptr()) }
            .ok_or_else(|| {
                Error::new(ErrorCode::CapabilityUnsupported, "Tablet view unavailable")
            })?;
        let window = view.window().ok_or_else(|| {
            Error::new(
                ErrorCode::CapabilityUnsupported,
                "Tablet view has no window",
            )
        })?;
        let epoch = EPOCH
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "Tablet installation epoch exhausted",
                )
            })?;
        if emitter.records.is_some() && NSEvent::pressedMouseButtons() != 0 {
            return Err(Error::new(
                ErrorCode::TargetInvalid,
                "Lift mouse buttons before starting ordered native capture",
            ));
        }
        emitter.record_epoch.store(epoch, Ordering::Release);
        let state = Arc::new(Mutex::new(State::new(epoch)?));
        let closed = Arc::new(AtomicBool::new(false));
        let recovery = Arc::new(recovery::Recovery::default());
        let callback_recovery = recovery.clone();
        let callback_state = state.clone();
        let callback_closed = closed.clone();
        let callback_emitter = emitter.clone();
        let callback_window = window.clone();
        let block = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit provides a live event for this local-monitor call.
            let event_ref = unsafe { event.as_ref() };
            let owned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if callback_closed.load(Ordering::Acquire) {
                    return false;
                }
                let owned = process(
                    event_ref,
                    &view,
                    &callback_window,
                    &callback_state,
                    &callback_emitter,
                    mtm,
                );
                // This is an explicit recovery observation, not a replacement
                // for button edges or per-event queued mouse history.
                if callback_emitter.records.is_some()
                    && callback_recovery.observe_idle(
                        callback_window.isKeyWindow(),
                        NSEvent::pressedMouseButtons(),
                    )
                {
                    callback_emitter.observe(CaptureEvent::Boundary {
                        kind: Boundary::PointerIdle,
                        time: None,
                    });
                }
                owned
            }))
            .unwrap_or_else(|_| {
                callback_emitter.source_failure();
                true
            });
            if owned {
                std::ptr::null_mut()
            } else {
                event.as_ptr()
            }
        });
        let mut mask = NSEventMask::TabletPoint
            | NSEventMask::TabletProximity
            | NSEventMask::LeftMouseDown
            | NSEventMask::LeftMouseUp
            | NSEventMask::LeftMouseDragged
            | NSEventMask::RightMouseDown
            | NSEventMask::RightMouseUp
            | NSEventMask::RightMouseDragged
            | NSEventMask::OtherMouseDown
            | NSEventMask::OtherMouseUp
            | NSEventMask::OtherMouseDragged
            | NSEventMask::MouseMoved;
        if emitter.records.is_some() {
            mask |= NSEventMask::ScrollWheel
                | NSEventMask::MouseEntered
                | NSEventMask::MouseExited
                | NSEventMask::Magnify
                | NSEventMask::SmartMagnify
                | NSEventMask::Rotate
                | NSEventMask::Swipe
                | NSEventMask::KeyDown
                | NSEventMask::KeyUp
                | NSEventMask::FlagsChanged;
        }
        // SAFETY: block returns either AppKit's still-live input event or null.
        // AppKit copies the block and calls it only on the main thread.
        let token = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) }
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Cannot install tablet event monitor",
                )
            })?;
        let center = NSNotificationCenter::defaultCenter();
        let mut observers = Vec::new();
        for (notification, closing, window_only, gaining) in [
            (
                unsafe { NSWindowDidResignKeyNotification },
                false,
                true,
                false,
            ),
            (
                unsafe { NSWindowDidBecomeKeyNotification },
                false,
                true,
                true,
            ),
            (unsafe { NSWindowWillCloseNotification }, true, true, false),
            (
                unsafe { NSApplicationDidResignActiveNotification },
                false,
                false,
                false,
            ),
        ] {
            let state = state.clone();
            let closed = closed.clone();
            let emitter = emitter.clone();
            let recovery = recovery.clone();
            let block = RcBlock::new(move |_event: NonNull<NSNotification>| {
                if gaining {
                    if emitter.records.is_some()
                        && recovery.observe_idle(true, NSEvent::pressedMouseButtons())
                    {
                        emitter.observe(CaptureEvent::Boundary {
                            kind: Boundary::PointerIdle,
                            time: None,
                        });
                    }
                    return;
                }
                recovery.focus_lost();
                if closing {
                    closed.store(true, Ordering::Release);
                }
                emitter.observe(CaptureEvent::Boundary {
                    kind: if closing {
                        Boundary::WindowClosed
                    } else {
                        Boundary::FocusLost
                    },
                    time: None,
                });
                cancel(&state, &emitter);
            });
            // SAFETY: this notification block is Send+Sync (only Arc/Mutex,
            // atomics and thread-safe Sink/Wake). It captures no Cocoa/UI object.
            // None queue preserves native delivery; observer tokens are retained.
            observers.push(unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(notification),
                    if window_only {
                        Some(window.as_ref())
                    } else {
                        None
                    },
                    None,
                    &block,
                )
            });
        }
        Ok(Self {
            token,
            observers,
            center,
            _window: window,
            state,
            closed,
            emitter,
        })
    }
    pub(super) fn cancel(&self) {
        // Host cancellation/guard removal must never reenter Sink under a
        // delivery handoff. The safe receiver owns its own private retirement.
        fence(&self.state);
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        // SAFETY: tokens come from these exact APIs, remain retained, and the
        // Installation guard is !Send so retirement runs on the creating thread.
        unsafe {
            NSEvent::removeMonitor(&self.token);
            for observer in &self.observers {
                let observer: &ProtocolObject<dyn NSObjectProtocol> = observer;
                self.center.removeObserver(observer.as_ref());
            }
        }
        self.cancel();
        self.emitter.report.installed(false);
    }
}
fn process(
    event: &NSEvent,
    view: &NSView,
    window: &NSWindow,
    state: &Mutex<State>,
    emitter: &Emitter,
    mtm: MainThreadMarker,
) -> bool {
    let kind = event.r#type();
    let matches_window = event
        .window(mtm)
        .is_some_and(|w| std::ptr::eq(w.as_ref(), window));
    if matches!(
        kind,
        NSEventType::KeyDown | NSEventType::KeyUp | NSEventType::FlagsChanged
    ) {
        if emitter.records.is_some() && (matches_window || window.isKeyWindow()) {
            emitter.observe(CaptureEvent::Boundary {
                kind: Boundary::Keyboard,
                time: native_time(event, emitter),
            });
        }
        return false; // No characters, key interpretation or IME are consumed.
    }
    let tablet_mouse = matches!(
        kind,
        NSEventType::LeftMouseDown
            | NSEventType::LeftMouseUp
            | NSEventType::LeftMouseDragged
            | NSEventType::RightMouseDown
            | NSEventType::RightMouseUp
            | NSEventType::RightMouseDragged
            | NSEventType::OtherMouseDown
            | NSEventType::OtherMouseUp
            | NSEventType::OtherMouseDragged
            | NSEventType::MouseMoved
    );
    let proximity = kind == NSEventType::TabletProximity
        || (tablet_mouse && event.subtype() == NSEventSubtype::TabletProximity);
    let is_point = kind == NSEventType::TabletPoint
        || (tablet_mouse && event.subtype() == NSEventSubtype::TabletPoint);
    // Local app-wide proximity metadata remains observed while inactive.
    if !matches_window && !proximity {
        return false;
    }
    if !proximity && !is_point {
        if emitter.records.is_some() {
            observe_mouse(event, view, window, emitter);
        }
        return false; // Genuine mouse, native chrome and gestures stay native.
    }
    let device = event.deviceID() as u64;
    if proximity {
        if !event.isEnteringProximity() {
            let packet = state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .leave(device);
            if let Some(packet) = packet {
                emitter.emit(packet);
            }
            return false;
        }
        let tool = match event.pointingDeviceType() {
            NSPointingDeviceType::Pen => DeviceKind::Pen,
            NSPointingDeviceType::Eraser => DeviceKind::Eraser,
            _ => return false,
        };
        let pointer = if event.uniqueID() != 0 {
            event.uniqueID()
        } else {
            event.pointingDeviceID() as u64
        };
        let caps = event.capabilityMask() as u64;
        if !emitter.report.device(Device {
            id: device,
            kind: tool,
            advertised: Axes {
                pressure: caps & 0x400 != 0,
                tilt: caps & 0x180 == 0x180,
                twist: caps & 0x2000 != 0,
                contact: false,
            },
            observed: Axes::default(),
            native_capabilities: caps,
        }) {
            return false;
        }
        let (_, packet) = state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .proximity(device, pointer, tool, caps);
        if let Some(packet) = packet {
            emitter.emit(packet);
        }
        return false;
    }
    if !view
        .window()
        .is_some_and(|current| std::ptr::eq(current.as_ref(), window))
    {
        return false;
    }
    let (position, in_view) = geometry(event, view, window);
    let Some(micros) = crate::clock::uptime_micros(event.timestamp()) else {
        let (consume, packet) = state.lock().unwrap_or_else(|e| e.into_inner()).quarantine(
            device,
            event.buttonMask().0 & 7 != 0,
            in_view,
        );
        if let Some(packet) = packet {
            emitter.emit(packet);
        }
        return consume;
    };
    let flags = event.modifierFlags();
    let native = event.buttonMask().0 as u16;
    // AppKit's distinct tip/lower-side/upper-side bits match bits 1/2/4.
    let buttons = native & 7;
    let tilt = event.tilt();
    let point = Point {
        device,
        in_view,
        micros,
        position,
        buttons,
        modifiers: Modifiers {
            shift: flags.contains(NSEventModifierFlags::Shift),
            control: flags.contains(NSEventModifierFlags::Control),
            alt: flags.contains(NSEventModifierFlags::Option),
            command: flags.contains(NSEventModifierFlags::Command),
        },
        pressure: event.pressure(),
        tilt: (tilt.x as f32, tilt.y as f32),
        rotation: event.rotation(),
    };
    dispatch(state, emitter, point)
}
fn native_time(event: &NSEvent, emitter: &Emitter) -> Option<paintcraft_input::tablet::Time> {
    Some(paintcraft_input::tablet::Time {
        epoch: emitter.record_epoch.load(Ordering::Acquire),
        micros: crate::clock::uptime_micros(event.timestamp())?,
        clock: paintcraft_input::tablet::Clock::AppKitUptime,
    })
}
fn native_modifiers(event: &NSEvent) -> Modifiers {
    let flags = event.modifierFlags();
    Modifiers {
        shift: flags.contains(NSEventModifierFlags::Shift),
        control: flags.contains(NSEventModifierFlags::Control),
        alt: flags.contains(NSEventModifierFlags::Option),
        command: flags.contains(NSEventModifierFlags::Command),
    }
}
fn observe_mouse(event: &NSEvent, view: &NSView, window: &NSWindow, emitter: &Emitter) {
    if !view
        .window()
        .is_some_and(|w| std::ptr::eq(w.as_ref(), window))
    {
        return;
    }
    let event_type = event.r#type();
    if let Some(boundary) = navigation_boundary(event_type, native_time(event, emitter)) {
        emitter.observe(boundary);
        return; // Preserve the original gesture for the framework; do not invent motion.
    }
    let kind = match event_type {
        NSEventType::LeftMouseDown
        | NSEventType::RightMouseDown
        | NSEventType::OtherMouseDown
        | NSEventType::LeftMouseUp
        | NSEventType::RightMouseUp
        | NSEventType::OtherMouseUp => {
            let Ok(number) = u8::try_from(event.buttonNumber()) else {
                emitter.source_failure();
                return;
            };
            if number >= 32 {
                emitter.source_failure();
                return;
            }
            MouseKind::Button {
                number,
                pressed: matches!(
                    event_type,
                    NSEventType::LeftMouseDown
                        | NSEventType::RightMouseDown
                        | NSEventType::OtherMouseDown
                ),
            }
        }
        NSEventType::MouseExited => MouseKind::Leave,
        NSEventType::ScrollWheel => {
            let momentum = event.momentumPhase();
            let phase = match momentum {
                NSEventPhase::MayBegin | NSEventPhase::Began => WheelPhase::Start,
                NSEventPhase::Ended | NSEventPhase::Cancelled => WheelPhase::End,
                _ => match event.phase() {
                    NSEventPhase::MayBegin | NSEventPhase::Began => WheelPhase::Start,
                    NSEventPhase::Ended | NSEventPhase::Cancelled => WheelPhase::End,
                    _ => WheelPhase::Move,
                },
            };
            let bounds = view.bounds();
            let scale = view.convertRectToBacking(bounds).size.width / bounds.size.width;
            MouseKind::Wheel {
                delta: (event.scrollingDeltaX(), event.scrollingDeltaY()),
                precise: event.hasPreciseScrollingDeltas(),
                physical_scale: scale,
                phase,
                momentum: momentum != NSEventPhase::None,
            }
        }
        NSEventType::MouseMoved
        | NSEventType::LeftMouseDragged
        | NSEventType::RightMouseDragged
        | NSEventType::OtherMouseDragged
        | NSEventType::MouseEntered => MouseKind::Motion,
        _ => return,
    };
    let Some(time) = native_time(event, emitter) else {
        emitter.source_failure();
        return;
    };
    let (position, client) = geometry(event, view, window);
    emitter.observe(CaptureEvent::Mouse(MouseRecord {
        kind,
        position,
        client,
        modifiers: native_modifiers(event),
        time,
    }));
}
fn navigation_boundary(
    kind: NSEventType,
    time: Option<paintcraft_input::tablet::Time>,
) -> Option<CaptureEvent> {
    matches!(
        kind,
        NSEventType::Magnify | NSEventType::SmartMagnify | NSEventType::Rotate | NSEventType::Swipe
    )
    .then_some(CaptureEvent::Boundary {
        kind: Boundary::Navigation,
        time,
    })
}
fn geometry(event: &NSEvent, view: &NSView, window: &NSWindow) -> (WindowPosition, bool) {
    let location = event.locationInWindow();
    let point = view.convertPoint_fromView(location, None);
    let view_bounds = view.bounds();
    let layout = window.contentLayoutRect();
    // NSView::hitTest expects its superview's coordinates, unlike bounds.
    // SAFETY: the retained window owns the currently attached view hierarchy;
    // this read and immediate retain occur on AppKit's main thread, with no
    // native event pumping or host callback between the attachment check/read.
    let parent = unsafe { view.superview() };
    let hit_point = parent.map_or(point, |parent| parent.convertPoint_fromView(location, None));
    // Client geometry is independent of key/focus status. An inactive client
    // contact must still be receiver-routed, never relabelled as native chrome.
    // The safe host applies captured focus permission separately.
    let in_view = location.x >= layout.origin.x
        && location.y >= layout.origin.y
        && location.x < layout.origin.x + layout.size.width
        && location.y < layout.origin.y + layout.size.height
        && point.x >= view_bounds.origin.x
        && point.y >= view_bounds.origin.y
        && point.x < view_bounds.origin.x + view_bounds.size.width
        && point.y < view_bounds.origin.y + view_bounds.size.height
        && view
            .hitTest(hit_point)
            .is_some_and(|hit| std::ptr::eq(hit.as_ref(), view));
    let point = view.convertPointToBacking(point);
    let bounds = view.convertRectToBacking(view_bounds);
    let x = point.x - bounds.origin.x;
    let y = if view.isFlipped() {
        point.y - bounds.origin.y
    } else {
        bounds.origin.y + bounds.size.height - point.y
    };
    (WindowPosition::PhysicalPixels(x, y), in_view)
}
fn dispatch(state: &Mutex<State>, emitter: &Emitter, point: Point) -> bool {
    let action = state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .prepare(point);
    match action {
        Action::Native => false,
        Action::Suppress => true,
        Action::Unclassified { consume } => {
            emitter.report.unclassified();
            emitter.wake();
            consume
        }
        Action::Dispatch(packet) => {
            emitter.emit(packet);
            // All classified pen events use receiver UI fallback. Their native
            // mouse promotion must never reach legacy canvas tool dispatch.
            true
        }
    }
}
fn fence(state: &Mutex<State>) {
    state.lock().unwrap_or_else(|e| e.into_inner()).cancel();
}
fn cancel(state: &Mutex<State>, emitter: &Emitter) {
    let packets = state.lock().unwrap_or_else(|e| e.into_inner()).cancel();
    for packet in packets.into_iter().flatten() {
        emitter.emit(packet);
    }
}

#[cfg(test)]
mod tests;

mod recovery;
