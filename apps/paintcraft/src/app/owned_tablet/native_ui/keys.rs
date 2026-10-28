//! Pure Windows key planning. No egui input delivery or native consumption.
use super::*;
use eframe::egui::{Key, Modifiers};
use paintcraft_platform::tablet::WindowsKey;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Disposition {
    Key {
        key: Key,
        pressed: bool,
        repeat: bool,
        modifiers: Modifiers,
    },
    /// Actual Ctrl+Alt cannot establish whether this is AltGr or a shortcut.
    AmbiguousModifiers,
    NativeSystem,
    ImeOwned,
    Orphan,
    Unmapped,
    PolicyReset,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KeyPlan {
    pub evidence: Evidence,
    pub disposition: Disposition,
    /// Policy invalidation, not a fabricated native key release.
    pub reset_gui: bool,
}
#[derive(Debug)]
pub(super) struct Planned {
    pub keys: Vec<KeyPlan>,
    /// Combined replacement cost: key expansions + other decoded output,
    /// including component envelopes. This is not a separate key allowance.
    pub output_cost: usize,
}
#[derive(Debug)]
pub(super) struct PreparedSegment {
    pub decoded: Vec<Decoded>,
    pub planned: Planned,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TextDisposition {
    NativeSystem,
    Control,
    /// Actual character evidence still requires an exact semantic target and
    /// source Scope. It is never proof that an earlier key generated this text.
    RequiresTarget,
}
pub(super) fn text_disposition(value: &Value) -> Option<TextDisposition> {
    match value {
        Value::Text { system: true, .. } => Some(TextDisposition::NativeSystem),
        Value::Text { scalar, .. } if scalar.is_control() => Some(TextDisposition::Control),
        Value::Text { .. } => Some(TextDisposition::RequiresTarget),
        _ => None,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Held {
    raw: WindowsKey,
    disposition: Disposition,
    fenced: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct KeyState {
    installation: u64,
    ordinal: Option<u64>,
    layout: Option<u64>,
    focused: bool,
    held: [Option<Held>; 512],
}
impl Default for KeyState {
    fn default() -> Self {
        Self {
            installation: 0,
            ordinal: None,
            layout: None,
            // Installation must acquire a real native focus fact before any GUI key.
            focused: false,
            held: [None; 512],
        }
    }
}
impl KeyState {
    /// The combined API does not commit Decoder's UTF16/composition state if
    /// key planning, expansion admission, or its allocation later refuses.
    pub fn decode_segment(
        &mut self,
        decoder: &mut decode::Decoder,
        input: &[Observation],
    ) -> Result<PreparedSegment, Fault> {
        let mut next_decoder = *decoder;
        let decoded = next_decoder.segment(input)?;
        let planned = self.segment(&decoded)?;
        *decoder = next_decoder;
        Ok(PreparedSegment { decoded, planned })
    }
    pub fn segment(&mut self, input: &[Decoded]) -> Result<Planned, Fault> {
        if input.len() > LIMIT {
            return Err(Fault::Capacity);
        }
        let mut next = self.clone();
        let mut keys = Vec::new();
        let mut cost = 0usize;
        // Preflight all expansion costs before allocating or changing state.
        for item in input {
            let amount = match &item.value {
                Value::Fact(NativeUi::WindowsKey(k)) => k.repeat_count as usize,
                Value::Text { scalar, repeat, .. } => scalar.len_utf16() * *repeat as usize,
                Value::Component { values, .. } => {
                    values.len().checked_add(2).ok_or(Fault::Capacity)?
                }
                Value::Fact(_) => 1,
            };
            cost = cost
                .checked_add(amount)
                .filter(|n| *n <= LIMIT)
                .ok_or(Fault::Capacity)?;
        }
        reserve(&mut keys, cost)?;
        for item in input {
            next.observe(item.evidence)?;
            match item.value {
                Value::Fact(NativeUi::WindowsKey(k)) => {
                    let (disposition, reset_gui) = next.key(k)?;
                    if reset_gui {
                        cost = cost
                            .checked_add(1)
                            .filter(|n| *n <= LIMIT)
                            .ok_or(Fault::Capacity)?;
                    }
                    for index in 0..k.repeat_count {
                        let disposition = match disposition {
                            Disposition::Key {
                                key,
                                pressed,
                                modifiers,
                                ..
                            } => Disposition::Key {
                                key,
                                pressed,
                                modifiers,
                                repeat: k.pressed && (k.was_down || index > 0),
                            },
                            other => other,
                        };
                        keys.push(KeyPlan {
                            evidence: item.evidence,
                            disposition,
                            reset_gui: reset_gui && index == 0,
                        });
                    }
                }
                Value::Fact(NativeUi::WindowsFocus { focused, .. }) => {
                    next.focused = focused;
                    if !focused {
                        next.fence();
                    }
                    keys.push(KeyPlan {
                        evidence: item.evidence,
                        disposition: Disposition::PolicyReset,
                        reset_gui: true,
                    });
                }
                Value::Fact(NativeUi::WindowsLayout { keyboard_layout }) => {
                    if keyboard_layout == 0 {
                        return Err(Fault::Order);
                    }
                    next.layout = Some(keyboard_layout);
                    next.fence();
                    keys.push(KeyPlan {
                        evidence: item.evidence,
                        disposition: Disposition::PolicyReset,
                        reset_gui: true,
                    });
                }
                _ => {}
            }
        }
        *self = next;
        Ok(Planned {
            keys,
            output_cost: cost,
        })
    }
    fn observe(&mut self, evidence: Evidence) -> Result<(), Fault> {
        let first = evidence.first;
        let last = evidence.last;
        if first.installation == 0
            || first.installation == u64::MAX
            || last.installation != first.installation
            || last.ordinal == u64::MAX
            || first.ordinal > last.ordinal
            || self.ordinal.is_some_and(|n| first.ordinal <= n)
            || (self.installation != 0 && self.installation != first.installation)
        {
            return Err(Fault::Order);
        }
        self.installation = first.installation;
        self.ordinal = Some(last.ordinal);
        Ok(())
    }
    fn fence(&mut self) {
        for held in self.held.iter_mut().flatten() {
            held.fenced = true;
        }
    }
    fn key(&mut self, raw: WindowsKey) -> Result<(Disposition, bool), Fault> {
        if raw.virtual_key == 0
            || raw.virtual_key > 0xfe
            || raw.repeat_count == 0
            || raw.keyboard_layout == 0
            || (!raw.pressed && (!raw.was_down || raw.repeat_count != 1))
        {
            return Err(Fault::Order);
        }
        if self
            .layout
            .is_some_and(|layout| layout != raw.keyboard_layout)
        {
            return Err(Fault::Order); // Require the actual layout lifecycle fact.
        }
        self.layout = Some(raw.keyboard_layout);
        let current = classify(raw);
        let priority_reset = matches!(
            current,
            Disposition::NativeSystem | Disposition::AmbiguousModifiers
        ) && self
            .held
            .iter()
            .flatten()
            .any(|held| !held.fenced && matches!(held.disposition, Disposition::Key { .. }));
        // Zero scan code has no demonstrable physical identity for this ledger.
        if raw.scan_code == 0 {
            if priority_reset {
                self.fence();
                return Ok((current, true));
            }
            return Ok((Disposition::Orphan, false));
        }
        let index = raw.scan_code as usize + usize::from(raw.extended) * 256;
        let old = self.held[index];
        let mapped = if raw.pressed {
            if let Some(held) = old {
                if !raw.was_down
                    || held.raw.virtual_key != raw.virtual_key
                    || held.raw.keyboard_layout != raw.keyboard_layout
                {
                    return Err(Fault::Order);
                }
                held
            } else {
                let held = Held {
                    raw,
                    disposition: classify(raw),
                    fenced: raw.was_down || !self.focused,
                };
                self.held[index] = Some(held);
                held
            }
        } else {
            let Some(held) = old else {
                if priority_reset {
                    self.fence();
                    return Ok((current, true));
                }
                return Ok((Disposition::Orphan, false));
            };
            if held.raw.virtual_key != 0xe5 && held.raw.virtual_key != raw.virtual_key {
                return Err(Fault::Order);
            }
            self.held[index] = None;
            held
        };
        if priority_reset {
            // A different actual key can activate OS menu/shortcut priority too.
            // Retire only still-held GUI keys; prior completed plans stay intact.
            self.fence();
            return Ok((current, true));
        }
        if mapped.fenced {
            return Ok((Disposition::Orphan, false));
        }
        Ok((
            match mapped.disposition {
                Disposition::Key { key, .. } => Disposition::Key {
                    key,
                    pressed: raw.pressed,
                    repeat: raw.was_down,
                    modifiers: modifiers(raw),
                },
                other => other,
            },
            false,
        ))
    }
}
fn modifiers(raw: WindowsKey) -> Modifiers {
    Modifiers {
        alt: raw.modifiers.alt,
        ctrl: raw.modifiers.control,
        shift: raw.modifiers.shift,
        mac_cmd: false,
        command: raw.modifiers.control,
    }
}
fn classify(raw: WindowsKey) -> Disposition {
    if raw.virtual_key == 0xe5 {
        return Disposition::ImeOwned;
    }
    // DefWindowProc retains F10 menu activation even for nonsystem KEYDOWN.
    if raw.virtual_key == 0x79 || raw.system_key || raw.alt_context || raw.modifiers.command {
        return Disposition::NativeSystem;
    }
    if raw.modifiers.control && raw.modifiers.alt {
        return Disposition::AmbiguousModifiers;
    }
    if raw.modifiers.alt {
        return Disposition::NativeSystem;
    }
    match map(raw.virtual_key) {
        Some(key) => Disposition::Key {
            key,
            pressed: raw.pressed,
            repeat: raw.was_down,
            modifiers: modifiers(raw),
        },
        None => Disposition::Unmapped,
    }
}
fn map(vk: u16) -> Option<Key> {
    Some(match vk {
        0x08 => Key::Backspace,
        0x09 => Key::Tab,
        0x0d => Key::Enter,
        0x1b => Key::Escape,
        0x20 => Key::Space,
        0x21 => Key::PageUp,
        0x22 => Key::PageDown,
        0x23 => Key::End,
        0x24 => Key::Home,
        0x25 => Key::ArrowLeft,
        0x26 => Key::ArrowUp,
        0x27 => Key::ArrowRight,
        0x28 => Key::ArrowDown,
        0x2d => Key::Insert,
        0x2e => Key::Delete,
        0x30..=0x39 => [
            Key::Num0,
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ][(vk - 0x30) as usize],
        0x41..=0x5a => [
            Key::A,
            Key::B,
            Key::C,
            Key::D,
            Key::E,
            Key::F,
            Key::G,
            Key::H,
            Key::I,
            Key::J,
            Key::K,
            Key::L,
            Key::M,
            Key::N,
            Key::O,
            Key::P,
            Key::Q,
            Key::R,
            Key::S,
            Key::T,
            Key::U,
            Key::V,
            Key::W,
            Key::X,
            Key::Y,
            Key::Z,
        ][(vk - 0x41) as usize],
        0x70..=0x87 => [
            Key::F1,
            Key::F2,
            Key::F3,
            Key::F4,
            Key::F5,
            Key::F6,
            Key::F7,
            Key::F8,
            Key::F9,
            Key::F10,
            Key::F11,
            Key::F12,
            Key::F13,
            Key::F14,
            Key::F15,
            Key::F16,
            Key::F17,
            Key::F18,
            Key::F19,
            Key::F20,
            Key::F21,
            Key::F22,
            Key::F23,
            Key::F24,
        ][(vk - 0x70) as usize],
        _ => return None,
    })
}
