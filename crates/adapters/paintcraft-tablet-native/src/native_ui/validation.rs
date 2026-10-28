//! Transactional source validation. No GUI event is authored here.
use super::*;
pub(crate) const MAX_CALLBACK_RECORDS: usize = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiFault {
    Capacity,
    Order,
    Text,
    Composition,
    Unsupported,
}
#[derive(Clone, Copy, Debug)]
struct Composition {
    id: u64,
    ended: bool,
    cancelled: bool,
}
#[derive(Clone, Copy, Debug)]
struct Tail {
    repeat: u16,
    system: bool,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct State {
    last: u64,
    composition: Option<Composition>,
    tail: Option<Tail>,
}
impl State {
    pub fn next_composition(&self) -> Result<u64, UiFault> {
        self.last
            .checked_add(1)
            .filter(|n| *n != u64::MAX)
            .ok_or(UiFault::Capacity)
    }
    pub fn composition(&self) -> Result<u64, UiFault> {
        self.composition.map(|c| c.id).ok_or(UiFault::Composition)
    }
    pub fn validate_callback(
        &mut self,
        records: &[NativeUi],
        envelope: usize,
    ) -> Result<(), UiFault> {
        if records
            .len()
            .checked_add(envelope)
            .ok_or(UiFault::Capacity)?
            > MAX_CALLBACK_RECORDS
        {
            return Err(UiFault::Capacity);
        }
        let mut next = self.clone();
        let mut index = 0;
        while index < records.len() {
            match records[index] {
                NativeUi::WindowsKey(key) => {
                    if next.tail.is_some()
                        || key.virtual_key == 0
                        || key.virtual_key > 0xfe
                        || key.repeat_count == 0
                        || key.keyboard_layout == 0
                        || (!key.pressed && !key.was_down)
                    {
                        return Err(UiFault::Order);
                    }
                    if key.repeat_count as usize > MAX_CALLBACK_RECORDS {
                        return Err(UiFault::Capacity);
                    }
                }
                NativeUi::WindowsCharacter(character) => next.character(character)?,
                NativeUi::WindowsDeadCharacter {
                    value,
                    repeat_count,
                    ..
                } => {
                    if next.tail.is_some()
                        || value > u16::MAX as u32
                        || char::from_u32(value).is_none()
                        || repeat_count == 0
                    {
                        return Err(UiFault::Text);
                    }
                    if repeat_count as usize > MAX_CALLBACK_RECORDS {
                        return Err(UiFault::Capacity);
                    }
                }
                NativeUi::WindowsFocus { .. } => {
                    if next.tail.is_some() {
                        return Err(UiFault::Text);
                    }
                }
                NativeUi::WindowsLayout { keyboard_layout } => {
                    if next.tail.is_some() || keyboard_layout == 0 {
                        return Err(UiFault::Order);
                    }
                }
                NativeUi::ImeBegin { composition } => {
                    if next.tail.is_some() || composition != next.next_composition()? {
                        return Err(UiFault::Composition);
                    }
                    next.last = composition;
                    next.composition = Some(Composition {
                        id: composition,
                        ended: false,
                        cancelled: false,
                    });
                }
                NativeUi::ImeEnd { composition } => {
                    let current = next
                        .composition
                        .as_mut()
                        .filter(|c| c.id == composition)
                        .ok_or(UiFault::Composition)?;
                    current.ended = true;
                }
                NativeUi::ImeNativeCancel { composition } => {
                    let current = next
                        .composition
                        .as_mut()
                        .filter(|c| c.id == composition)
                        .ok_or(UiFault::Composition)?;
                    current.cancelled = true;
                }
                NativeUi::ImeFrameStart {
                    composition,
                    component,
                    count,
                    cursor_native,
                    native_flags,
                } => {
                    if next.tail.is_some() || native_flags == 0 {
                        return Err(UiFault::Composition);
                    }
                    let current = next
                        .composition
                        .filter(|c| c.id == composition && !c.cancelled)
                        .ok_or(UiFault::Composition)?;
                    if current.ended
                        && !matches!(
                            component,
                            ImeComponent::ResultUtf16 | ImeComponent::WindowsResultClauses
                        )
                    {
                        return Err(UiFault::Composition);
                    }
                    if cursor_native.is_some() && component != ImeComponent::PreeditUtf16 {
                        return Err(UiFault::Order);
                    }
                    let start = index.checked_add(1).ok_or(UiFault::Capacity)?;
                    let end = start.checked_add(count as usize).ok_or(UiFault::Capacity)?;
                    let data = records.get(start..end).ok_or(UiFault::Order)?;
                    for (offset, entry) in data.iter().enumerate() {
                        let NativeUi::ImeData { index, value } = entry else {
                            return Err(UiFault::Order);
                        };
                        if *index as usize != offset {
                            return Err(UiFault::Order);
                        }
                        match component {
                            ImeComponent::ResultUtf16 | ImeComponent::PreeditUtf16
                                if *value > u16::MAX as u32 =>
                            {
                                return Err(UiFault::Text);
                            }
                            ImeComponent::WindowsAttributes if *value > 5 => {
                                return Err(UiFault::Text);
                            }
                            _ => {}
                        }
                    }
                    if !matches!(records.get(end), Some(NativeUi::ImeFrameEnd { composition: id }) if *id == composition)
                    {
                        return Err(UiFault::Order);
                    }
                    if matches!(
                        component,
                        ImeComponent::ResultUtf16 | ImeComponent::PreeditUtf16
                    ) {
                        let units = data.iter().map(|entry| match entry {
                            NativeUi::ImeData { value, .. } => *value as u16,
                            _ => unreachable!(),
                        });
                        if char::decode_utf16(units).any(|c| c.is_err()) {
                            return Err(UiFault::Text);
                        }
                    }
                    index = end;
                }
                NativeUi::ImeData { .. } | NativeUi::ImeFrameEnd { .. } => {
                    return Err(UiFault::Order);
                }
                NativeUi::UnsupportedImeCharacter { .. } => return Err(UiFault::Unsupported),
            }
            index += 1;
        }
        *self = next;
        Ok(())
    }
    fn character(&mut self, c: WindowsCharacter) -> Result<(), UiFault> {
        if c.repeat_count == 0 {
            return Err(UiFault::Text);
        }
        match c.kind {
            WindowsCharacterKind::Unichar => {
                let scalar = char::from_u32(c.value).ok_or(UiFault::Text)?;
                if self.tail.is_some() || c.system_character || c.value == 0xffff {
                    return Err(UiFault::Text);
                }
                if scalar.len_utf16() * c.repeat_count as usize > MAX_CALLBACK_RECORDS {
                    return Err(UiFault::Capacity);
                }
            }
            WindowsCharacterKind::Utf16 => {
                let unit: u16 = c.value.try_into().map_err(|_| UiFault::Text)?;
                if let Some(tail) = self.tail {
                    if !(0xdc00..=0xdfff).contains(&unit)
                        || tail.repeat != c.repeat_count
                        || tail.system != c.system_character
                    {
                        return Err(UiFault::Text);
                    }
                    self.tail = None;
                } else if (0xd800..=0xdbff).contains(&unit) {
                    if 2 * c.repeat_count as usize > MAX_CALLBACK_RECORDS {
                        return Err(UiFault::Capacity);
                    }
                    self.tail = Some(Tail {
                        repeat: c.repeat_count,
                        system: c.system_character,
                    });
                } else if (0xdc00..=0xdfff).contains(&unit) {
                    return Err(UiFault::Text);
                } else if c.repeat_count as usize > MAX_CALLBACK_RECORDS {
                    return Err(UiFault::Capacity);
                }
            }
        }
        Ok(())
    }
}
/// Raw W-API buffers are validated before any frame is staged for a sink.
pub(crate) fn validate_snapshot(
    result: &[u16],
    preedit: &[u16],
    attributes: &[u8],
    preedit_clauses: &[u32],
    result_clauses: &[u32],
) -> Result<(), UiFault> {
    for text in [result, preedit] {
        if char::decode_utf16(text.iter().copied()).any(|c| c.is_err()) {
            return Err(UiFault::Text);
        }
    }
    if !attributes.is_empty()
        && (attributes.len() != preedit.len() || attributes.iter().any(|a| *a > 5))
    {
        return Err(UiFault::Text);
    }
    for (clauses, text) in [(preedit_clauses, preedit), (result_clauses, result)] {
        if clauses.is_empty() {
            continue;
        }
        if clauses[0] != 0
            || clauses.last().copied() != Some(text.len() as u32)
            || clauses.windows(2).any(|p| p[0] > p[1])
        {
            return Err(UiFault::Text);
        }
        for offset in clauses.iter().map(|n| *n as usize) {
            if offset > text.len()
                || (offset > 0
                    && offset < text.len()
                    && (0xd800..=0xdbff).contains(&text[offset - 1])
                    && (0xdc00..=0xdfff).contains(&text[offset]))
            {
                return Err(UiFault::Text);
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests;
