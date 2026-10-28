//! Source-native UI facts for a canonical owned host. These contain no GUI
//! target: the ordered coordinator must bind composition ownership at Begin.
use paintcraft_input::Modifiers;

/// Raw Windows keyboard evidence; no inferred text or physical device ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowsKey {
    pub virtual_key: u16,
    pub scan_code: u8,
    pub extended: bool,
    pub pressed: bool,
    pub repeat_count: u16,
    pub was_down: bool,
    pub system_key: bool,
    pub alt_context: bool,
    /// Opaque native HKL observed during acquisition, not a language guess.
    pub keyboard_layout: u64,
    pub modifiers: Modifiers,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsCharacterKind {
    /// An actual WM_CHAR/WM_SYSCHAR UTF16 code unit. Surrogate tails require
    /// complete-stream validation before any GUI text can be authored.
    Utf16,
    /// One validated WM_UNICHAR scalar, never its UNICODE_NOCHAR query.
    Unichar,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowsCharacter {
    pub kind: WindowsCharacterKind,
    pub value: u32,
    pub repeat_count: u16,
    pub system_character: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImeComponent {
    ResultUtf16,
    PreeditUtf16,
    /// Raw IMM bytes, not a fabricated GUI selection.
    WindowsAttributes,
    WindowsPreeditClauses,
    WindowsResultClauses,
}
/// Copy payloads keep bounded native record admission independent of GUI types.
/// A frame's data ordinals serialize one native observation; they are not
/// separate physical key events. Source callbacks have no invented timestamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeUi {
    WindowsKey(WindowsKey),
    WindowsCharacter(WindowsCharacter),
    WindowsDeadCharacter {
        value: u32,
        system: bool,
        repeat_count: u16,
    },
    WindowsFocus {
        focused: bool,
        modifiers: Modifiers,
    },
    WindowsLayout {
        keyboard_layout: u64,
    },
    ImeBegin {
        composition: u64,
    },
    /// Actual native END, not policy cancellation or a speculative commit.
    ImeEnd {
        composition: u64,
    },
    /// Actual COMPOSITION with no GCS fields, distinct from coordinator policy.
    ImeNativeCancel {
        composition: u64,
    },
    ImeFrameStart {
        composition: u64,
        component: ImeComponent,
        count: u16,
        /// Raw SDK cursor result; no guessed UTF16/scalar conversion.
        cursor_native: Option<u32>,
        native_flags: u32,
    },
    ImeData {
        index: u16,
        value: u32,
    },
    ImeFrameEnd {
        composition: u64,
    },
    /// Retained native fact which MUST retire delivery without authored text.
    UnsupportedImeCharacter {
        value: u32,
        flags: u32,
    },
}

#[cfg(any(target_os = "windows", test))]
mod validation;
#[cfg(target_os = "windows")]
pub(crate) use validation::{MAX_CALLBACK_RECORDS, State, UiFault, validate_snapshot};
