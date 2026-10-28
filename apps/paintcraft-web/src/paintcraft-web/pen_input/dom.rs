//! Pointer Events acquisition, with synchronous user activation retained for
//! widget presses/releases. Reported pressure is not a hardware capability claim.
use super::{
    CAPACITY,
    acquisition::Acquisition,
    events::{Compatibility, Kind, Normalizer, Pair, Pointer},
};
use crate::WebApp;
use paintcraft_input::{
    Modifiers,
    tablet::{Invalid, Provenance, receiver::Route},
};
use paintcraft_ui::pen_input::Source;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Mutex},
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Event, EventTarget, HtmlCanvasElement, MouseEvent, PointerEvent};

type Listener = (EventTarget, &'static str, Closure<dyn FnMut(Event)>);
/// Reentrant cancellation cannot depend on WebRunner::try_lock. These small
/// capture/lifecycle listeners are independently removed before runner teardown.
pub(crate) struct Guard {
    listeners: [Option<Listener>; 5],
    abort: web_sys::AbortController,
    source: Source,
    canvas: HtmlCanvasElement,
    captured: Rc<Cell<Option<i32>>>,
    acquisition: Arc<Mutex<Acquisition>>,
}
impl Drop for Guard {
    fn drop(&mut self) {
        // eframe0.37 removes managed listeners without the capture flag. Abort
        // these capture registrations before it drops their Rust closures.
        self.abort.abort();
        self.source.cancel();
        self.acquisition
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reset(&self.source);
        if let Some(id) = self.captured.take() {
            let _ = self.canvas.release_pointer_capture(id);
        }
        for (target, name, listener) in self.listeners.iter_mut().filter_map(Option::take) {
            let _ = target.remove_event_listener_with_callback_and_bool(
                name,
                listener.as_ref().unchecked_ref(),
                true,
            );
        }
    }
}
fn listener(
    target: &EventTarget,
    name: &'static str,
    closure: impl FnMut(Event) + 'static,
) -> Result<Listener, JsValue> {
    let closure = Closure::wrap(Box::new(closure) as Box<dyn FnMut(Event)>);
    target.add_event_listener_with_callback_and_bool(
        name,
        closure.as_ref().unchecked_ref(),
        true,
    )?;
    Ok((target.clone(), name, closure))
}
fn number(event: &PointerEvent, key: &str) -> Result<f64, Invalid> {
    js_sys::Reflect::get(event, &JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_f64())
        .filter(|v| v.is_finite())
        .ok_or(Invalid::Coordinates)
}
fn mouse_modifiers(event: &MouseEvent) -> eframe::egui::Modifiers {
    super::modifiers(Modifiers {
        shift: event.shift_key(),
        control: event.ctrl_key(),
        alt: event.alt_key(),
        command: event.meta_key(),
    })
}
fn content_rect(canvas: &HtmlCanvasElement) -> (f64, f64, f64, f64) {
    let rect = canvas.get_bounding_client_rect();
    let (mut left, mut top, mut right, mut bottom) =
        (rect.left(), rect.top(), rect.right(), rect.bottom());
    if let Some(window) = web_sys::window()
        && let Ok(Some(style)) = window.get_computed_style(canvas)
    {
        let padding = |name| {
            style
                .get_property_value(name)
                .ok()
                .and_then(|s| s.trim_end_matches("px").parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        left += padding("padding-left");
        top += padding("padding-top");
        right -= padding("padding-right");
        bottom -= padding("padding-bottom");
    }
    (left, top, right, bottom)
}
fn mouse_point(
    event: &MouseEvent,
    canvas: &HtmlCanvasElement,
    ctx: &eframe::egui::Context,
) -> eframe::egui::Pos2 {
    // Preserve the pinned eframe0.37 mouse policy (integer client coordinates,
    // CSS content-box origin and GUI zoom). Pen coordinates retain fractions.
    let (left, top, _, _) = content_rect(canvas);
    eframe::egui::pos2(
        (event.client_x() as f32 - left as f32) / ctx.zoom_factor(),
        (event.client_y() as f32 - top as f32) / ctx.zoom_factor(),
    )
}
fn pointer(event: &PointerEvent, kind: Kind) -> Pointer {
    Pointer {
        id: event.pointer_id() as u32 as u64,
        kind,
        at: None,
        stamp: event.time_stamp(),
        buttons: event.buttons(),
        pressure: event.pressure(),
        tilt: (event.tilt_x() as f32, event.tilt_y() as f32),
        twist: event.twist() as f32,
        contact: (event.width() as f32, event.height() as f32),
        modifiers: Modifiers {
            shift: event.shift_key(),
            control: event.ctrl_key(),
            alt: event.alt_key(),
            command: event.meta_key(),
        },
    }
}
fn submit(
    source: &Source,
    acquisition: &Mutex<Acquisition>,
    normalizer: &RefCell<Normalizer>,
    event: &PointerEvent,
    kind: Kind,
    canvas: &HtmlCanvasElement,
    provenance: Provenance,
) -> Result<Route, Invalid> {
    let pointer = pointer(event, kind);
    let epoch = source.cancellation_epoch();
    let mut state = normalizer.borrow_mut();
    state.observe_cancellation(epoch);
    let fenced = state.contact_fenced(pointer.id);
    let mut raw = state.sample(pointer, provenance)?;
    drop(state);
    acquisition
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .pen(&mut raw)?;
    let route = super::events::deliver(source, raw, fenced, |ppp| {
        let (left, top, right, bottom) = content_rect(canvas);
        if right <= left || bottom <= top {
            return Err(Invalid::Coordinates);
        }
        Ok(Some((
            (number(event, "clientX")? - left) * f64::from(canvas.width()) / (right - left) / ppp,
            (number(event, "clientY")? - top) * f64::from(canvas.height()) / (bottom - top) / ppp,
        )))
    })?;
    if route == Route::Suppressed && raw.buttons.0 != 0 {
        normalizer.borrow_mut().deny_contact(raw.identity.pointer);
    }
    Ok(route)
}
fn coalesced(event: &PointerEvent) -> Result<js_sys::Array, Invalid> {
    let method = js_sys::Reflect::get(event, &JsValue::from_str("getCoalescedEvents"))
        .map_err(|_| Invalid::Lifecycle)?;
    if method.is_undefined() {
        return Ok(js_sys::Array::new());
    }
    let function = method
        .dyn_into::<js_sys::Function>()
        .map_err(|_| Invalid::Lifecycle)?;
    let value = function.call0(event).map_err(|_| Invalid::Lifecycle)?;
    if !js_sys::Array::is_array(&value) {
        return Err(Invalid::Lifecycle);
    }
    let array = js_sys::Array::from(&value);
    if array.length() as usize >= CAPACITY {
        return Err(Invalid::Capacity);
    }
    Ok(array)
}
pub(crate) fn install(
    runner: &eframe::WebRunner,
    canvas: HtmlCanvasElement,
) -> Result<Guard, JsValue> {
    let app = runner
        .app_mut::<WebApp>()
        .ok_or_else(|| JsValue::from_str("Pen editor missing"))?;
    let source = app
        .pen
        .source
        .clone()
        .ok_or_else(|| JsValue::from_str("Pen storage unavailable"))?;
    let acquisition = app
        .pen
        .acquisition
        .clone()
        .ok_or_else(|| JsValue::from_str("Pointer order storage unavailable"))?;
    drop(app);
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("Browser window missing"))?;
    let epoch = window
        .performance()
        .map_or(0, |p| p.time_origin().to_bits());
    let normalizer = Rc::new(RefCell::new(Normalizer::new(epoch)));
    let compatibility = Rc::new(RefCell::new(Compatibility::default()));
    let captured = Rc::new(Cell::new(None));
    let ctx = runner
        .app_mut::<WebApp>()
        .ok_or_else(|| JsValue::from_str("Pen editor missing"))?
        .pen_context();
    let target: EventTarget = canvas.clone().unchecked_into();
    let mut guard = Guard {
        listeners: std::array::from_fn(|_| None),
        abort: web_sys::AbortController::new()?,
        source: source.clone(),
        canvas: canvas.clone(),
        captured: captured.clone(),
        acquisition: acquisition.clone(),
    };
    for (index, name) in ["pointercancel", "lostpointercapture"]
        .into_iter()
        .enumerate()
    {
        let source = source.clone();
        let captured = captured.clone();
        let ctx = ctx.clone();
        let normalizer = normalizer.clone();
        guard.listeners[index] = Some(listener(&target, name, move |event| {
            let Some(event) = event.dyn_ref::<PointerEvent>() else {
                return;
            };
            if event.pointer_type() != "pen" || captured.get() != Some(event.pointer_id()) {
                return;
            }
            captured.set(None);
            normalizer.borrow_mut().clear();
            source.cancel();
            event.stop_immediate_propagation();
            ctx.request_repaint();
        })?);
    }
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("Browser document missing"))?;
    let document_target: EventTarget = document.clone().unchecked_into();
    let window_target: EventTarget = window.unchecked_into();
    for (index, name) in ["blur", "resize"].into_iter().enumerate() {
        let source = source.clone();
        let ctx = ctx.clone();
        let captured = captured.clone();
        let canvas = canvas.clone();
        let normalizer = normalizer.clone();
        let acquisition = acquisition.clone();
        let window_identity = window_target.clone();
        guard.listeners[index + 2] = Some(listener(&window_target, name, move |event| {
            if name == "blur" && event.target().as_ref() != Some(&window_identity) {
                return;
            }
            normalizer.borrow_mut().clear();
            acquisition
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .reset(&source);
            if name == "resize" {
                source.invalidate_view();
            } else {
                source.cancel();
            }
            if let Some(id) = captured.take() {
                let _ = canvas.release_pointer_capture(id);
            }
            ctx.request_repaint();
        })?);
    }
    {
        let source = source.clone();
        let ctx = ctx.clone();
        let captured = captured.clone();
        let normalizer = normalizer.clone();
        let acquisition = acquisition.clone();
        let document = document.clone();
        let canvas = canvas.clone();
        guard.listeners[4] = Some(listener(&document_target, "visibilitychange", move |_| {
            if !document.hidden() {
                return;
            }
            normalizer.borrow_mut().clear();
            acquisition
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .reset(&source);
            source.cancel();
            if let Some(id) = captured.take() {
                let _ = canvas.release_pointer_capture(id);
            }
            ctx.request_repaint();
        })?);
    }
    let options = web_sys::AddEventListenerOptions::new();
    options.set_capture(true);
    options.set_passive(false);
    options.set_signal(&guard.abort.signal());
    for (name, kind) in [
        ("pointerdown", Kind::Down),
        ("pointermove", Kind::Move),
        ("pointerup", Kind::Up),
    ] {
        let source = source.clone();
        let acquisition = acquisition.clone();
        let compatibility = compatibility.clone();
        let canvas = canvas.clone();
        let document = document.clone();
        runner.add_event_listener_ex(
            &document_target,
            name,
            &options,
            move |event: PointerEvent, runner, _| {
                if event.pointer_type() != "mouse" || !event.is_trusted() {
                    return;
                }
                compatibility.borrow_mut().mouse();
                let interested = document
                    .element_from_point(event.client_x() as f32, event.client_y() as f32)
                    .is_some_and(|element| element.eq(&canvas))
                    || runner
                        .egui_ctx()
                        .input(|i| i.pointer.any_down() || i.any_touches());
                let previous = acquisition
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .buttons();
                let accepted = acquisition
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .capture_mouse(&source, event.buttons() as u8);
                if !accepted {
                    event.prevent_default();
                    event.stop_immediate_propagation();
                    compatibility.borrow_mut().pen(Pair {
                        kind,
                        stamp: event.time_stamp().to_bits(),
                        buttons: event.buttons(),
                        trusted: true,
                    });
                    return;
                }
                if !interested {
                    return;
                }
                let changed = previous ^ event.buttons() as u8;
                if changed == 0 {
                    return;
                }
                let ctx = runner.egui_ctx().clone();
                let pos = mouse_point(&event, &canvas, &ctx);
                // Every changed physical button gets its matching GUI edge,
                // including chord changes delivered as pointermove by the DOM.
                event.stop_immediate_propagation();
                if !runner.has_focus() {
                    let options = web_sys::FocusOptions::new();
                    options.set_prevent_scroll(true);
                    let _ = canvas.focus_with_options(&options);
                    runner.update_focus();
                }
                runner.logic();
                let result = acquisition
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .button_changes(
                        &source,
                        previous,
                        event.buttons() as u8,
                        pos,
                        mouse_modifiers(&event),
                    );
                if !matches!(result, Ok(true)) {
                    source.cancel();
                    runner
                        .app_mut::<WebApp>()
                        .pen_failure(&ctx, "mouse acquisition refused");
                }
                if kind == Kind::Up {
                    let options = web_sys::FocusOptions::new();
                    options.set_prevent_scroll(true);
                    let _ = canvas.focus_with_options(&options);
                    runner.update_focus();
                }
                runner.logic();
                ctx.request_repaint();
            },
        )?;
    }
    // Flush earlier owned pointer work before eframe translates the next key or
    // IME event. Its event objects and keyboard/IME translator remain unchanged.
    for name in [
        "keydown",
        "keyup",
        "beforeinput",
        "compositionstart",
        "compositionupdate",
        "compositionend",
    ] {
        let acquisition = acquisition.clone();
        runner.add_event_listener_ex(
            &document_target,
            name,
            &options,
            move |_: Event, runner, _| {
                let flush = acquisition
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .needs_flush();
                if flush {
                    runner.logic();
                }
            },
        )?;
    }
    for (name, kind) in [
        ("pointerdown", Kind::Down),
        ("pointermove", Kind::Move),
        ("pointerup", Kind::Up),
        ("pointerleave", Kind::Leave),
    ] {
        let source = source.clone();
        let normalizer = normalizer.clone();
        let compatibility = compatibility.clone();
        let canvas = canvas.clone();
        let captured = captured.clone();
        let acquisition = acquisition.clone();
        runner.add_event_listener_ex(
            &target,
            name,
            &options,
            move |event: PointerEvent, runner, _| {
                if event.pointer_type() != "pen" {
                    if event.pointer_type() != "mouse" {
                        compatibility.borrow_mut().mouse();
                    }
                    return;
                }
                normalizer
                    .borrow_mut()
                    .observe_cancellation(source.cancellation_epoch());
                let first_contact = kind == Kind::Down
                    || normalizer
                        .borrow()
                        .starting(event.pointer_id() as u32 as u64, event.buttons());
                if first_contact || kind == Kind::Up {
                    if first_contact && !runner.has_focus() {
                        let options = web_sys::FocusOptions::new();
                        options.set_prevent_scroll(true);
                        let _ = canvas.focus_with_options(&options);
                        runner.update_focus();
                    }
                    runner.logic();
                }
                // Never pass classified pen contacts to eframe's mouse translator.
                event.prevent_default();
                event.stop_immediate_propagation();
                if kind != Kind::Leave {
                    compatibility.borrow_mut().pen(Pair {
                        kind,
                        stamp: event.time_stamp().to_bits(),
                        buttons: event.buttons(),
                        trusted: event.is_trusted(),
                    });
                }
                let result = (|| {
                    if kind == Kind::Move {
                        for value in coalesced(&event)?.iter() {
                            let sample = value
                                .dyn_into::<PointerEvent>()
                                .map_err(|_| Invalid::Lifecycle)?;
                            if sample.pointer_id() != event.pointer_id()
                                || sample.pointer_type() != "pen"
                            {
                                return Err(Invalid::Owner);
                            }
                            submit(
                                &source,
                                &acquisition,
                                &normalizer,
                                &sample,
                                kind,
                                &canvas,
                                Provenance::Coalesced,
                            )?;
                        }
                    }
                    submit(
                        &source,
                        &acquisition,
                        &normalizer,
                        &event,
                        kind,
                        &canvas,
                        Provenance::Actual,
                    )
                })();
                match result {
                    Ok(route) => {
                        if event.buttons() != 0
                            && matches!(route, Route::Canvas | Route::Ui)
                            && captured.get().is_none()
                        {
                            if canvas.set_pointer_capture(event.pointer_id()).is_err() {
                                source.cancel();
                                normalizer.borrow_mut().clear();
                            } else {
                                captured.set(Some(event.pointer_id()));
                            }
                        }
                        if event.buttons() == 0 && captured.get() == Some(event.pointer_id()) {
                            // A normal release must not cancel the queued valid Up.
                            captured.set(None);
                            let _ = canvas.release_pointer_capture(event.pointer_id());
                        }
                    }
                    Err(error) => {
                        source.cancel();
                        normalizer.borrow_mut().clear();
                        let ctx = runner.egui_ctx().clone();
                        runner
                            .app_mut::<WebApp>()
                            .pen_failure(&ctx, format!("{error:?}"));
                    }
                }
                runner.egui_ctx().request_repaint();
                if matches!(kind, Kind::Down | Kind::Up) {
                    runner.logic();
                }
            },
        )?;
    }
    for (name, kind) in [
        ("mousedown", Kind::Down),
        ("mousemove", Kind::Move),
        ("mouseup", Kind::Up),
    ] {
        let compatibility = compatibility.clone();
        let acquisition = acquisition.clone();
        let source = source.clone();
        let canvas = canvas.clone();
        let document = document.clone();
        runner.add_event_listener_ex(
            &document_target,
            name,
            &options,
            move |event: MouseEvent, runner, _| {
                if compatibility.borrow_mut().matches(Pair {
                    kind,
                    stamp: event.time_stamp().to_bits(),
                    buttons: event.buttons(),
                    trusted: event.is_trusted(),
                }) {
                    event.prevent_default();
                    event.stop_immediate_propagation();
                    return;
                }
                if kind != Kind::Move || !event.is_trusted() {
                    return;
                }
                let interested = document
                    .element_from_point(event.client_x() as f32, event.client_y() as f32)
                    .is_some_and(|element| element.eq(&canvas))
                    || runner
                        .egui_ctx()
                        .input(|i| i.pointer.any_down() || i.any_touches());
                if !interested {
                    return;
                }
                let ctx = runner.egui_ctx().clone();
                let gui = eframe::egui::Event::PointerMoved(mouse_point(&event, &canvas, &ctx));
                event.stop_immediate_propagation();
                let result = acquisition.lock().unwrap_or_else(|e| e.into_inner()).mouse(
                    &source,
                    gui,
                    event.buttons() as u8,
                    mouse_modifiers(&event),
                );
                if let Err(error) = result {
                    source.cancel();
                    runner
                        .app_mut::<WebApp>()
                        .pen_failure(&ctx, format!("mouse motion acquisition refused: {error:?}"));
                }
                ctx.request_repaint();
            },
        )?;
    }
    Ok(guard)
}
