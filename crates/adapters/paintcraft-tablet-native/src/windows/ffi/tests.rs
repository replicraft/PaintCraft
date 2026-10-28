//! These controls are headless; none creates a HWND or injects native input.
use super::*;
#[test]
fn retained_callback_context_cannot_acquire_another_installations_authority() {
    let inner = Rc::new(Inner::default());
    let old = Context {
        inner: inner.clone(),
        installation: 19,
    };
    assert!(!old.current());
    inner.installation.set(19);
    inner.active.set(true);
    assert!(old.current());
    inner.active.set(false);
    assert!(!old.current());
    inner.installation.set(20);
    inner.active.set(true);
    let new = Context {
        inner: inner.clone(),
        installation: 20,
    };
    assert!(!old.current());
    assert!(new.current());
    // An uncertain native removal fences both callbacks and further reuse.
    inner.retirement_failed.set(true);
    assert!(!old.current());
    assert!(!new.current());
}
#[test]
fn copied_flag_meanings_match_the_selected_windows_sdk() {
    assert_eq!(model::IN_RANGE, POINTER_FLAG_INRANGE.0);
    assert_eq!(model::CONTACT, POINTER_FLAG_INCONTACT.0);
    assert_eq!(model::PRIMARY, POINTER_FLAG_PRIMARY.0);
    assert_eq!(model::DOWN, POINTER_FLAG_DOWN.0);
    assert_eq!(model::UPDATE, POINTER_FLAG_UPDATE.0);
    assert_eq!(model::UP, POINTER_FLAG_UP.0);
    assert_eq!(model::CONFIDENCE, POINTER_FLAG_CONFIDENCE.0);
    assert_eq!(model::CANCELED, POINTER_FLAG_CANCELED.0);
    assert_eq!(model::CAPTURE_CHANGED, POINTER_FLAG_CAPTURECHANGED.0);
    assert_eq!(model::PRESSURE, PEN_MASK_PRESSURE);
    assert_eq!(model::ROTATION, PEN_MASK_ROTATION);
    assert_eq!(model::TILT_X, PEN_MASK_TILT_X);
    assert_eq!(model::TILT_Y, PEN_MASK_TILT_Y);
    assert_eq!(model::BARREL, PEN_FLAG_BARREL);
    assert_eq!(model::ERASER, PEN_FLAG_ERASER);
    assert_eq!(model::INVERTED, PEN_FLAG_INVERTED);
    assert!(pointer_flags_match(
        POINTER_FLAG_INRANGE.0,
        POINTER_MESSAGE_FLAG_INRANGE
    ));
    assert!(!pointer_flags_match(POINTER_FLAG_INRANGE.0, 0));
    assert!(!pointer_flags_match(POINTER_FLAG_INCONTACT.0, 0));
}
#[test]
fn scratch_admission_refuses_before_allocating_and_releases_after_storage() {
    let budget = SharedBudget::new(1);
    assert!(Storage::new(&budget).is_err());
    assert_eq!(budget.usage().reserved, 0);
    let budget = SharedBudget::new(16 << 20);
    let scratch = Storage::new(&budget).unwrap();
    assert!(scratch.history.capacity() >= LIMIT);
    assert!(scratch.touch_history.capacity() >= LIMIT);
    assert!(scratch.events.capacity() >= LIMIT);
    assert_eq!(scratch._account.bytes(), budget.usage().reserved);
    drop(scratch);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn device_report_storage_remains_charged_until_the_final_report_clone_drops() {
    let budget = SharedBudget::new(1 << 20);
    let report = Report::new(Backend::WindowsPointer);
    report.reserve_devices(&budget).unwrap();
    let reserved = budget.usage().reserved;
    assert!(reserved > 0);
    for id in 1..=64 {
        assert!(report.device(crate::Device {
            id,
            kind: paintcraft_input::tablet::DeviceKind::Pen,
            advertised: crate::Axes::default(),
            observed: crate::Axes::default(),
            native_capabilities: 0
        }));
    }
    assert!(!report.device(crate::Device {
        id: 65,
        kind: paintcraft_input::tablet::DeviceKind::Pen,
        advertised: crate::Axes::default(),
        observed: crate::Axes::default(),
        native_capabilities: 0
    }));
    assert_eq!(budget.usage().reserved, reserved);
    let survivor = report.clone();
    drop(report);
    assert_eq!(budget.usage().reserved, reserved);
    drop(survivor);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn genuine_mouse_dispatch_does_not_treat_keyboard_or_unrecognized_messages_as_leave() {
    use crate::MouseKind;
    assert_eq!(mouse::classify(WM_KEYDOWN, 0).unwrap(), None);
    assert_eq!(mouse::classify(WM_PAINT, 0).unwrap(), None);
    assert_eq!(
        mouse::classify(windows::Win32::UI::Controls::WM_MOUSELEAVE, 0).unwrap(),
        Some(MouseKind::Leave)
    );
    for (message, button, pressed, wparam) in [
        (WM_LBUTTONDOWN, 0, true, 0),
        (WM_RBUTTONUP, 1, false, 0),
        (WM_MBUTTONDOWN, 2, true, 0),
        (WM_XBUTTONDOWN, 3, true, 1usize << 16),
        (WM_XBUTTONUP, 4, false, 2usize << 16),
    ] {
        assert_eq!(
            mouse::classify(message, wparam).unwrap(),
            Some(MouseKind::Button {
                number: button,
                pressed
            })
        );
    }
    assert!(mouse::classify(WM_XBUTTONDOWN, 3usize << 16).is_err());
    for delta in [120i16, -120] {
        for (message, expected) in [
            (WM_MOUSEWHEEL, (0.0, delta as f64 / 120.0)),
            (WM_MOUSEHWHEEL, (-delta as f64 / 120.0, 0.0)),
        ] {
            assert!(matches!(
                mouse::classify(message, (delta as u16 as usize) << 16).unwrap(),
                Some(MouseKind::Wheel { delta, .. }) if delta == expected
            ));
        }
    }
}

#[test]
fn single_native_control_keeps_metadata_without_querying_or_inventing_client_geometry() {
    let pointer = POINTER_INFO {
        pointerId: 7,
        frameId: 41,
        pointerFlags: POINTER_FLAG_CANCELED,
        historyCount: 1,
        PerformanceCount: 99,
        ptPixelLocationRaw: POINT {
            x: 90_000,
            y: -40_000,
        },
        ButtonChangeType: POINTER_CHANGE_SECONDBUTTON_UP,
        ..POINTER_INFO::default()
    };
    let copied = control::metadata(
        pointer,
        Message::Update,
        paintcraft_input::Modifiers::default(),
    )
    .unwrap();
    assert_eq!(copied.button_change, POINTER_CHANGE_SECONDBUTTON_UP.0);
    assert_eq!(copied.performance, 99);
    assert_eq!(copied.pointer, 7);
    assert!(!copied.client);
    assert!(
        control::metadata(
            POINTER_INFO {
                historyCount: 2,
                ..pointer
            },
            Message::Update,
            Default::default()
        )
        .is_none()
    );
    assert!(
        control::metadata(
            POINTER_INFO {
                pointerFlags: POINTER_FLAG_UPDATE,
                ..pointer
            },
            Message::Update,
            Default::default()
        )
        .is_none()
    );
    // A real Up retains its endpoint/history requirements while it can commit.
    assert!(
        control::metadata(
            POINTER_INFO {
                pointerFlags: POINTER_FLAG_UP,
                ..pointer
            },
            Message::Up,
            Default::default()
        )
        .is_none()
    );
}
