use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Decoder {
    installation: u64,
    ordinal: Option<u64>,
    last_composition: u64,
    composition: Option<(u64, bool, bool)>, // actual End, actual Cancel
    tail: Option<Observation>,
}
impl Decoder {
    /// Commit parser state only after every record and every output allocation
    /// succeeds. Callers cannot obtain a valid prefix of a refused segment.
    pub fn segment(&mut self, input: &[Observation]) -> Result<Vec<Decoded>, Fault> {
        if input.len() > LIMIT {
            return Err(Fault::Capacity);
        }
        let mut next = *self;
        let mut output = Vec::new();
        reserve(&mut output, input.len())?;
        let mut expanded = 0usize;
        let mut index = 0;
        while index < input.len() {
            let first = input[index];
            next.observe(first)?;
            let mut last = first;
            let value = match first.fact {
                NativeUi::WindowsCharacter(c) => {
                    let repeat = c.repeat_count as usize;
                    if repeat == 0 {
                        return Err(Fault::Text);
                    }
                    let scalar = match c.kind {
                        WindowsCharacterKind::Unichar => {
                            if next.tail.is_some() || c.system_character || c.value == 0xffff {
                                return Err(Fault::Text);
                            }
                            char::from_u32(c.value).ok_or(Fault::Text)?
                        }
                        WindowsCharacterKind::Utf16 => {
                            let unit = u16::try_from(c.value).map_err(|_| Fault::Text)?;
                            if let Some(high) = next.tail.take() {
                                let NativeUi::WindowsCharacter(h) = high.fact else {
                                    unreachable!()
                                };
                                if !(0xdc00..=0xdfff).contains(&unit)
                                    || h.repeat_count != c.repeat_count
                                    || h.system_character != c.system_character
                                {
                                    return Err(Fault::Text);
                                }
                                let scalar =
                                    0x10000 + ((h.value - 0xd800) << 10) + (unit as u32 - 0xdc00);
                                expanded =
                                    expanded.checked_add(repeat * 2).ok_or(Fault::Capacity)?;
                                if expanded > LIMIT {
                                    return Err(Fault::Capacity);
                                }
                                output.push(Decoded {
                                    evidence: Evidence { first: high, last },
                                    value: Value::Text {
                                        scalar: char::from_u32(scalar).ok_or(Fault::Text)?,
                                        repeat: c.repeat_count,
                                        system: c.system_character,
                                    },
                                });
                                index += 1;
                                continue;
                            } else if (0xd800..=0xdbff).contains(&unit) {
                                if repeat * 2 > LIMIT {
                                    return Err(Fault::Capacity);
                                }
                                next.tail = Some(first);
                                index += 1;
                                continue;
                            } else {
                                char::from_u32(unit as u32).ok_or(Fault::Text)?
                            }
                        }
                    };
                    expanded = expanded
                        .checked_add(scalar.len_utf16() * repeat)
                        .ok_or(Fault::Capacity)?;
                    Value::Text {
                        scalar,
                        repeat: c.repeat_count,
                        system: c.system_character,
                    }
                }
                NativeUi::ImeFrameStart {
                    composition,
                    component,
                    count,
                    cursor_native,
                    native_flags,
                } => {
                    next.no_tail()?;
                    let (_, ended, cancelled) = next
                        .composition
                        .filter(|c| c.0 == composition)
                        .ok_or(Fault::Composition)?;
                    if cancelled
                        || native_flags == 0
                        || (ended
                            && !matches!(
                                component,
                                ImeComponent::ResultUtf16 | ImeComponent::WindowsResultClauses
                            ))
                        || (cursor_native.is_some() && component != ImeComponent::PreeditUtf16)
                    {
                        return Err(Fault::Composition);
                    }
                    let end = index
                        .checked_add(count as usize + 1)
                        .ok_or(Fault::Capacity)?;
                    let data = input.get(index + 1..end).ok_or(Fault::Order)?;
                    let mut values = Vec::new();
                    reserve(&mut values, data.len())?;
                    for (offset, entry) in data.iter().copied().enumerate() {
                        next.observe(entry)?;
                        let NativeUi::ImeData { index, value } = entry.fact else {
                            return Err(Fault::Order);
                        };
                        if index as usize != offset {
                            return Err(Fault::Order);
                        }
                        if component == ImeComponent::WindowsAttributes && value > 5 {
                            return Err(Fault::Text);
                        }
                        values.push(value);
                    }
                    last = *input.get(end).ok_or(Fault::Order)?;
                    next.observe(last)?;
                    if last.fact != (NativeUi::ImeFrameEnd { composition }) {
                        return Err(Fault::Order);
                    }
                    let text = if matches!(
                        component,
                        ImeComponent::PreeditUtf16 | ImeComponent::ResultUtf16
                    ) {
                        let mut text = String::new();
                        text.try_reserve_exact(values.len() * 3)
                            .map_err(|_| Fault::Capacity)?;
                        if values.iter().any(|v| *v > u16::MAX as u32) {
                            return Err(Fault::Text);
                        }
                        for scalar in char::decode_utf16(values.iter().map(|v| *v as u16)) {
                            text.push(scalar.map_err(|_| Fault::Text)?);
                        }
                        expanded = expanded.checked_add(values.len()).ok_or(Fault::Capacity)?;
                        Some(text)
                    } else {
                        None
                    };
                    index = end;
                    Value::Component {
                        composition,
                        kind: component,
                        values,
                        text,
                        cursor_native,
                        native_flags,
                    }
                }
                fact => {
                    next.no_tail()?;
                    match fact {
                        NativeUi::ImeBegin { composition } => {
                            if composition == 0
                                || composition == u64::MAX
                                || composition <= next.last_composition
                            {
                                return Err(Fault::Composition);
                            }
                            next.last_composition = composition;
                            next.composition = Some((composition, false, false));
                        }
                        NativeUi::ImeEnd { composition }
                        | NativeUi::ImeNativeCancel { composition } => {
                            let current = next
                                .composition
                                .as_mut()
                                .filter(|c| c.0 == composition)
                                .ok_or(Fault::Composition)?;
                            if matches!(fact, NativeUi::ImeEnd { .. }) {
                                current.1 = true;
                            } else {
                                current.2 = true;
                            }
                        }
                        NativeUi::WindowsKey(k) => {
                            if k.repeat_count == 0
                                || k.virtual_key == 0
                                || k.virtual_key > 0xfe
                                || k.keyboard_layout == 0
                                || (!k.pressed && !k.was_down)
                            {
                                return Err(Fault::Order);
                            }
                            expanded = expanded
                                .checked_add(k.repeat_count as usize)
                                .ok_or(Fault::Capacity)?;
                        }
                        NativeUi::WindowsDeadCharacter {
                            value,
                            repeat_count,
                            ..
                        } => {
                            if value > u16::MAX as u32
                                || char::from_u32(value).is_none()
                                || repeat_count == 0
                            {
                                return Err(Fault::Text);
                            }
                            expanded = expanded
                                .checked_add(repeat_count as usize)
                                .ok_or(Fault::Capacity)?;
                        }
                        NativeUi::WindowsLayout { keyboard_layout: 0 } => return Err(Fault::Order),
                        NativeUi::UnsupportedImeCharacter { .. } => return Err(Fault::Unsupported),
                        NativeUi::ImeData { .. } | NativeUi::ImeFrameEnd { .. } => {
                            return Err(Fault::Order);
                        }
                        _ => {}
                    }
                    Value::Fact(fact)
                }
            };
            if expanded > LIMIT {
                return Err(Fault::Capacity);
            }
            output.push(Decoded {
                evidence: Evidence { first, last },
                value,
            });
            index += 1;
        }
        // No complete prefix escapes beside an unresolved scalar. A lone
        // high surrogate can be retained with its original observation.
        if next.tail.is_some() && !output.is_empty() {
            return Err(Fault::Order);
        }
        *self = next;
        Ok(output)
    }
    fn observe(&mut self, record: Observation) -> Result<(), Fault> {
        if record.installation == 0
            || record.installation == u64::MAX
            || record.ordinal == u64::MAX
            || (self.installation != 0 && self.installation != record.installation)
            || self.ordinal.is_some_and(|last| record.ordinal <= last)
        {
            return Err(Fault::Order);
        }
        self.installation = record.installation;
        self.ordinal = Some(record.ordinal);
        Ok(())
    }
    fn no_tail(&self) -> Result<(), Fault> {
        if self.tail.is_some() {
            Err(Fault::Text)
        } else {
            Ok(())
        }
    }
}
