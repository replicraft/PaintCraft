use super::*;
#[test]
fn admitted_ui_storage_is_released_and_refused_before_native_registration() {
    let denied = SharedBudget::new(1);
    assert!(Capture::new(&denied).is_err());
    assert_eq!(denied.usage().reserved, 0);
    let budget = SharedBudget::new(16 << 20);
    let storage = Capture::new(&budget).unwrap();
    assert!(storage.records.capacity() >= MAX_CALLBACK_RECORDS);
    assert_eq!(budget.usage().reserved, storage._account.bytes());
    drop(storage);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn key_transition_validation_preserves_physical_and_system_fields() {
    let raw = keyboard::record(
        WM_SYSKEYUP,
        WPARAM(VK_F4.0 as usize),
        LPARAM((1u32 | (0x3e << 16) | (1 << 24) | (1 << 29) | (1 << 30) | (1 << 31)) as isize),
    )
    .unwrap();
    let NativeUi::WindowsKey(key) = raw else {
        panic!("key")
    };
    assert_eq!(key.scan_code, 0x3e);
    assert!(key.extended && key.system_key && key.alt_context && key.was_down);
    assert!(!key.pressed);
    assert!(keyboard::record(WM_KEYDOWN, WPARAM(65), LPARAM((1u32 | (1 << 31)) as isize)).is_err());
}
#[test]
fn unicode_query_and_ime_char_are_recognized_without_fabricating_text() {
    assert!(recognizes(WM_UNICHAR));
    assert!(recognizes(WM_IME_CHAR));
    assert!(!recognizes(WM_PAINT));
    assert_eq!(UNICODE_NOCHAR, 0xffff);
    let inner = Rc::new(Inner::default());
    let reply = unsafe {
        dispatch(
            &inner,
            HWND::default(),
            WM_UNICHAR,
            WPARAM(UNICODE_NOCHAR as usize),
            LPARAM(0),
        )
    };
    assert_eq!(reply.0, 1);
    assert!(inner.target.borrow().is_none());
}

#[test]
fn pretranslation_ui_messages_do_not_observe_recovery_before_hwnd_acquisition() {
    let inner = Rc::new(Inner::default());
    inner.active.set(true);
    let hwnd = HWND(std::ptr::dangling_mut());
    inner.hwnd.set(hwnd.0 as usize);
    // Any pretranslation acquisition would borrow this slot. The actual UI
    // callback must own its first source read, including later idle recovery.
    let _exclusive = inner.target.borrow_mut();
    let message = MSG {
        hwnd,
        message: WM_KEYDOWN,
        ..MSG::default()
    };
    assert!(!dispatch_hook(&inner, &message));
}
