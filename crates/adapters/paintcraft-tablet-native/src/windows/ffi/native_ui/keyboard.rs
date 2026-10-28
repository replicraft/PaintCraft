use super::*;
pub(super) fn record(message: u32, wparam: WPARAM, lparam: LPARAM) -> Result<NativeUi> {
    let bits = lparam.0 as u32;
    let repeat_count = (bits & 0xffff) as u16;
    let keyboard_layout = || unsafe { GetKeyboardLayout(0) }.0 as usize as u64;
    Ok(match message {
        WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP => {
            let pressed = matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN);
            if pressed == (bits & (1 << 31) != 0)
                || (!pressed && (repeat_count != 1 || bits & (1 << 30) == 0))
            {
                return Err(error("Native key transition contradicts message"));
            }
            NativeUi::WindowsKey(WindowsKey {
                virtual_key: wparam
                    .0
                    .try_into()
                    .map_err(|_| error("Native virtual key overflow"))?,
                scan_code: ((bits >> 16) & 0xff) as u8,
                extended: bits & (1 << 24) != 0,
                pressed,
                repeat_count,
                was_down: bits & (1 << 30) != 0,
                system_key: matches!(message, WM_SYSKEYDOWN | WM_SYSKEYUP),
                alt_context: bits & (1 << 29) != 0,
                keyboard_layout: keyboard_layout(),
                modifiers: mouse::modifiers(),
            })
        }
        WM_CHAR | WM_SYSCHAR | WM_UNICHAR => NativeUi::WindowsCharacter(WindowsCharacter {
            kind: if message == WM_UNICHAR {
                WindowsCharacterKind::Unichar
            } else {
                WindowsCharacterKind::Utf16
            },
            value: wparam
                .0
                .try_into()
                .map_err(|_| error("Native character overflow"))?,
            repeat_count,
            system_character: message == WM_SYSCHAR,
        }),
        WM_DEADCHAR | WM_SYSDEADCHAR => NativeUi::WindowsDeadCharacter {
            value: wparam
                .0
                .try_into()
                .map_err(|_| error("Native dead character overflow"))?,
            system: message == WM_SYSDEADCHAR,
            repeat_count,
        },
        WM_SETFOCUS | WM_KILLFOCUS => NativeUi::WindowsFocus {
            focused: message == WM_SETFOCUS,
            modifiers: mouse::modifiers(),
        },
        WM_INPUTLANGCHANGE => NativeUi::WindowsLayout {
            keyboard_layout: lparam.0 as usize as u64,
        },
        _ => return Err(error("Unsupported native keyboard message")),
    })
}
