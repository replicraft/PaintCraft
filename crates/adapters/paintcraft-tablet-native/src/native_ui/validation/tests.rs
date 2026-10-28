use super::*;
fn begin() -> NativeUi {
    NativeUi::ImeBegin { composition: 1 }
}
fn frame(component: ImeComponent, units: &[u32]) -> Vec<NativeUi> {
    let mut out = vec![NativeUi::ImeFrameStart {
        composition: 1,
        component,
        count: units.len() as u16,
        cursor_native: None,
        native_flags: 2048,
    }];
    out.extend(
        units
            .iter()
            .enumerate()
            .map(|(index, value)| NativeUi::ImeData {
                index: index as u16,
                value: *value,
            }),
    );
    out.push(NativeUi::ImeFrameEnd { composition: 1 });
    out
}
fn character(kind: WindowsCharacterKind, value: u32, repeat_count: u16) -> NativeUi {
    NativeUi::WindowsCharacter(WindowsCharacter {
        kind,
        value,
        repeat_count,
        system_character: false,
    })
}
#[test]
fn callback_failure_rolls_back_begin_and_every_valid_prefix() {
    let mut state = State::default();
    let mut records = vec![begin()];
    records.extend(frame(ImeComponent::ResultUtf16, &[65, 0xd800, 66]));
    assert_eq!(state.validate_callback(&records, 1), Err(UiFault::Text));
    assert_eq!(state.composition(), Err(UiFault::Composition));
    assert_eq!(state.next_composition().unwrap(), 1);
    state.validate_callback(&[begin()], 1).unwrap();
    assert_eq!(state.composition().unwrap(), 1);
}
#[test]
fn complete_frames_preserve_real_results_after_end_and_multiple_active_segments() {
    let mut state = State::default();
    state.validate_callback(&[begin()], 1).unwrap();
    state
        .validate_callback(&frame(ImeComponent::ResultUtf16, &[65]), 1)
        .unwrap();
    state
        .validate_callback(&frame(ImeComponent::PreeditUtf16, &[0x65e5]), 1)
        .unwrap();
    state
        .validate_callback(&frame(ImeComponent::ResultUtf16, &[0x65e5]), 1)
        .unwrap();
    state
        .validate_callback(&[NativeUi::ImeEnd { composition: 1 }], 1)
        .unwrap();
    state
        .validate_callback(&frame(ImeComponent::ResultUtf16, &[0xd83c, 0xdfa8]), 1)
        .unwrap();
    assert_eq!(
        state.validate_callback(&frame(ImeComponent::PreeditUtf16, &[65]), 1),
        Err(UiFault::Composition)
    );
}
#[test]
fn native_cancel_differs_from_end_and_rejects_later_result() {
    let mut state = State::default();
    state
        .validate_callback(
            &[
                begin(),
                NativeUi::ImeNativeCancel { composition: 1 },
                NativeUi::ImeEnd { composition: 1 },
            ],
            1,
        )
        .unwrap();
    assert_eq!(
        state.validate_callback(&frame(ImeComponent::ResultUtf16, &[65]), 1),
        Err(UiFault::Composition)
    );
    state
        .validate_callback(&[NativeUi::ImeBegin { composition: 2 }], 1)
        .unwrap();
    assert_eq!(state.composition().unwrap(), 2);
}
#[test]
fn missing_begin_end_index_or_wrong_composition_never_admits_a_frame() {
    let mut state = State::default();
    assert_eq!(
        state.validate_callback(&frame(ImeComponent::ResultUtf16, &[65]), 1),
        Err(UiFault::Composition)
    );
    state.validate_callback(&[begin()], 1).unwrap();
    let mut missing = frame(ImeComponent::ResultUtf16, &[65]);
    missing.pop();
    assert_eq!(state.validate_callback(&missing, 1), Err(UiFault::Order));
    let mut bad = frame(ImeComponent::ResultUtf16, &[65]);
    bad[1] = NativeUi::ImeData {
        index: 1,
        value: 65,
    };
    assert_eq!(state.validate_callback(&bad, 1), Err(UiFault::Order));
    let mut bad = frame(ImeComponent::ResultUtf16, &[65]);
    *bad.last_mut().unwrap() = NativeUi::ImeFrameEnd { composition: 2 };
    assert_eq!(state.validate_callback(&bad, 1), Err(UiFault::Order));
}
#[test]
fn callback_limit_counts_frame_headers_and_existing_controls() {
    let mut state = State::default();
    state.validate_callback(&[begin()], 1).unwrap();
    let too_large = frame(
        ImeComponent::ResultUtf16,
        &vec![65; MAX_CALLBACK_RECORDS - 2],
    );
    assert_eq!(
        state.validate_callback(&too_large, 1),
        Err(UiFault::Capacity)
    );
    let fits = frame(
        ImeComponent::ResultUtf16,
        &vec![65; MAX_CALLBACK_RECORDS - 3],
    );
    state.validate_callback(&fits, 1).unwrap();
}
#[test]
fn scalar_repeat_and_utf16_tails_are_checked_without_replacement_text() {
    let mut state = State::default();
    assert_eq!(
        state.validate_callback(&[character(WindowsCharacterKind::Unichar, 0xd800, 1)], 1),
        Err(UiFault::Text)
    );
    assert_eq!(
        state.validate_callback(
            &[character(WindowsCharacterKind::Unichar, 0x1f3a8, u16::MAX)],
            1
        ),
        Err(UiFault::Capacity)
    );
    state
        .validate_callback(&[character(WindowsCharacterKind::Utf16, 0xd83c, 1)], 1)
        .unwrap();
    assert_eq!(
        state.validate_callback(&[character(WindowsCharacterKind::Utf16, 65, 1)], 1),
        Err(UiFault::Text)
    );
    state
        .validate_callback(&[character(WindowsCharacterKind::Utf16, 0xdfa8, 1)], 1)
        .unwrap();
    assert_eq!(
        state.validate_callback(&[character(WindowsCharacterKind::Utf16, 0xdfa8, 1)], 1),
        Err(UiFault::Text)
    );
}
#[test]
fn focus_and_layout_cannot_silently_rebind_an_incomplete_character() {
    let mut state = State::default();
    state
        .validate_callback(&[character(WindowsCharacterKind::Utf16, 0xd83c, 1)], 1)
        .unwrap();
    assert_eq!(
        state.validate_callback(
            &[NativeUi::WindowsFocus {
                focused: false,
                modifiers: Modifiers::default()
            }],
            1
        ),
        Err(UiFault::Text)
    );
    assert_eq!(
        state.validate_callback(&[NativeUi::WindowsLayout { keyboard_layout: 1 }], 1),
        Err(UiFault::Order)
    );
    state
        .validate_callback(&[character(WindowsCharacterKind::Utf16, 0xdfa8, 1)], 1)
        .unwrap();
}
#[test]
fn raw_ime_char_is_explicitly_unsupported() {
    let mut state = State::default();
    assert_eq!(
        state.validate_callback(
            &[NativeUi::UnsupportedImeCharacter {
                value: 65,
                flags: 0
            }],
            1
        ),
        Err(UiFault::Unsupported)
    );
}

#[test]
fn unicode_capability_query_cannot_be_admitted_as_character_text() {
    let mut state = State::default();
    assert_eq!(
        state.validate_callback(&[character(WindowsCharacterKind::Unichar, 0xffff, 1)], 1),
        Err(UiFault::Text)
    );
    assert_eq!(state.composition(), Err(UiFault::Composition));
}
#[test]
fn snapshots_validate_attributes_clause_bounds_and_surrogate_boundaries() {
    validate_snapshot(&[65], &[0xd83c, 0xdfa8], &[1, 1], &[0, 2], &[0, 1]).unwrap();
    assert_eq!(
        validate_snapshot(&[], &[0xd83c], &[], &[], &[]),
        Err(UiFault::Text)
    );
    assert_eq!(
        validate_snapshot(&[], &[0xd83c, 0xdfa8], &[1], &[], &[]),
        Err(UiFault::Text)
    );
    assert_eq!(
        validate_snapshot(&[], &[0xd83c, 0xdfa8], &[1, 1], &[0, 1, 2], &[]),
        Err(UiFault::Text)
    );
    assert_eq!(
        validate_snapshot(&[], &[65], &[7], &[], &[]),
        Err(UiFault::Text)
    );
    assert_eq!(
        validate_snapshot(&[], &[65], &[1], &[0, 2], &[]),
        Err(UiFault::Text)
    );
}

#[test]
fn repeat_capacity_refusal_preserves_state_at_key_and_supplementary_scalar_boundaries() {
    let key = |repeat_count| {
        NativeUi::WindowsKey(WindowsKey {
            virtual_key: 65,
            scan_code: 30,
            extended: false,
            pressed: true,
            repeat_count,
            was_down: true,
            system_key: false,
            alt_context: false,
            keyboard_layout: 1,
            modifiers: Modifiers::default(),
        })
    };
    let mut state = State::default();
    assert_eq!(
        state.validate_callback(&[begin(), key(4097)], 1),
        Err(UiFault::Capacity)
    );
    assert_eq!(state.composition(), Err(UiFault::Composition));
    state.validate_callback(&[key(4096)], 1).unwrap();
    state
        .validate_callback(
            &[character(WindowsCharacterKind::Unichar, 0x1f3a8, 2048)],
            1,
        )
        .unwrap();
    assert_eq!(
        state.validate_callback(
            &[
                begin(),
                character(WindowsCharacterKind::Unichar, 0x1f3a8, 2049)
            ],
            1
        ),
        Err(UiFault::Capacity)
    );
    assert_eq!(state.composition(), Err(UiFault::Composition));
    assert_eq!(
        state.validate_callback(&[character(WindowsCharacterKind::Utf16, 65, 4097)], 1),
        Err(UiFault::Capacity)
    );
    assert_eq!(
        state.validate_callback(&[character(WindowsCharacterKind::Utf16, 0xdc00, 4097)], 1),
        Err(UiFault::Text)
    );
}

#[test]
fn dead_character_repeat_is_evidence_and_has_the_same_admission_limit() {
    let record = |repeat_count| NativeUi::WindowsDeadCharacter {
        value: 0xb4,
        system: false,
        repeat_count,
    };
    let mut state = State::default();
    assert_eq!(state.validate_callback(&[record(0)], 1), Err(UiFault::Text));
    assert_eq!(
        state.validate_callback(&[record(4097)], 1),
        Err(UiFault::Capacity)
    );
    state.validate_callback(&[record(4096)], 1).unwrap();
}
